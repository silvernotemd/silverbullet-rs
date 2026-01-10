use std::collections::HashMap;

use ::opendal::Operator;
use async_trait::async_trait;
use futures::StreamExt;

use super::utils::now;
use crate::fs::*;

pub struct Filesystem {
    operator: Operator,
}

impl Filesystem {
    pub fn new(operator: Operator) -> Self {
        Self { operator }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl ReadOnlyFilesystem for Filesystem {
    async fn list(&self) -> Result<Vec<FileMeta>> {
        Ok(self
            .operator
            .list_with("/")
            .recursive(true)
            .await?
            .iter()
            .map(FileMeta::from)
            .collect())
    }

    async fn get(&self, path: &str) -> Result<(Stream, FileMeta)> {
        let stat = self.operator.stat(path).await?;

        let stream = self
            .operator
            .reader(path)
            .await?
            .into_bytes_stream(..)
            .await?;

        use crate::fs::StreamExt;

        Ok((stream.into_boxed(), (path, stat).into()))
    }

    async fn meta(&self, path: &str) -> Result<FileMeta> {
        let stat = self.operator.stat(path).await?;

        Ok((path, stat).into())
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl WritableFilesystem for Filesystem {
    async fn put(&self, path: &str, mut data: Stream, meta: IncomingFileMeta) -> Result<FileMeta> {
        let mut options = ::opendal::options::WriteOptions {
            content_type: meta.content_type,
            ..Default::default()
        };

        if let Some(created) = meta.created {
            options.user_metadata = Some(HashMap::from([(
                "created".to_string(),
                created.to_string(),
            )]));
        }

        let mut writer = self.operator.writer_options(path, options).await?;

        while let Some(chunk) = data.next().await {
            writer.write(chunk?).await?;
        }

        writer.close().await?;

        let stat = self.operator.stat(path).await?;

        Ok((path, stat).into())
    }

    async fn delete(&self, path: &str) -> Result<()> {
        // Stat the file before so a Not Found error is returned if the file does not exist
        // This is required by the SilverBullet API
        self.operator.stat(path).await?;

        self.operator.delete(path).await?;

        Ok(())
    }
}

impl From<&::opendal::Entry> for FileMeta {
    fn from(entry: &::opendal::Entry) -> Self {
        let path = entry.path();
        let metadata = entry.metadata().clone();

        (path, metadata).into()
    }
}

impl From<(&str, ::opendal::Metadata)> for FileMeta {
    fn from((path, metadata): (&str, ::opendal::Metadata)) -> Self {
        FileMeta {
            name: path.to_string(),
            created: metadata
                .user_metadata()
                .and_then(|um| um.get("created"))
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(now),
            perm: "rw".to_string(), // Default to read-write for now
            content_type: metadata
                .content_type()
                .unwrap_or("application/octet-stream")
                .to_string(),
            last_modified: metadata
                .last_modified()
                .map(|lm| lm.into_inner().as_millisecond().unsigned_abs())
                .unwrap_or_else(now),
            size: metadata.content_length(),
        }
    }
}

impl From<::opendal::Error> for Error {
    fn from(err: ::opendal::Error) -> Self {
        match err.kind() {
            ::opendal::ErrorKind::NotFound => Error::NotFound(err.into()),
            ::opendal::ErrorKind::PermissionDenied => Error::PermissionDenied(err.into()),
            _ => Error::Other(err.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::opendal::services::Memory;
    use bytes::Bytes;
    use futures::stream;

    fn memory_fs() -> Filesystem {
        let op = Operator::new(Memory::default()).unwrap().finish();
        Filesystem::new(op)
    }

    fn bytes_stream(data: &[u8]) -> Stream {
        let bytes = Bytes::from(data.to_vec());
        Box::pin(stream::once(async move { Ok(bytes) }))
    }

    async fn collect_stream(stream: Stream) -> Vec<u8> {
        use futures::TryStreamExt;
        stream
            .try_fold(Vec::new(), |mut acc, chunk| async move {
                acc.extend_from_slice(&chunk);
                Ok(acc)
            })
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn list_empty() {
        let fs = memory_fs();
        let files = fs.list().await.unwrap();
        assert!(files.is_empty());
    }

    #[tokio::test]
    async fn put_and_get() {
        let fs = memory_fs();
        let content = b"hello world";

        let meta = fs
            .put(
                "test.txt",
                bytes_stream(content),
                IncomingFileMeta::default(),
            )
            .await
            .unwrap();

        assert_eq!(meta.name, "test.txt");
        assert_eq!(meta.size, content.len() as u64);

        let (stream, get_meta) = fs.get("test.txt").await.unwrap();
        assert_eq!(get_meta.name, "test.txt");

        let data = collect_stream(stream).await;
        assert_eq!(data, content);
    }

    #[tokio::test]
    async fn put_with_content_type() {
        let fs = memory_fs();

        let meta = IncomingFileMeta {
            content_type: Some("text/plain".to_string()),
            ..Default::default()
        };

        let result = fs
            .put("test.txt", bytes_stream(b"data"), meta)
            .await
            .unwrap();
        assert_eq!(result.content_type, "text/plain");
    }

    #[tokio::test]
    async fn put_with_created_timestamp() {
        let fs = memory_fs();

        let meta = IncomingFileMeta {
            created: Some(1234567890),
            ..Default::default()
        };

        // Note: Memory backend doesn't persist user metadata, so created falls back to now()
        let result = fs
            .put("test.txt", bytes_stream(b"data"), meta)
            .await
            .unwrap();
        assert!(result.created > 0);
    }

    #[tokio::test]
    async fn list_after_put() {
        let fs = memory_fs();

        fs.put("a.txt", bytes_stream(b"a"), IncomingFileMeta::default())
            .await
            .unwrap();
        fs.put("b.txt", bytes_stream(b"bb"), IncomingFileMeta::default())
            .await
            .unwrap();

        let files = fs.list().await.unwrap();
        assert_eq!(files.len(), 2);

        let names: Vec<_> = files.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"a.txt"));
        assert!(names.contains(&"b.txt"));
    }

    #[tokio::test]
    async fn meta_returns_file_info() {
        let fs = memory_fs();

        fs.put(
            "test.txt",
            bytes_stream(b"content"),
            IncomingFileMeta::default(),
        )
        .await
        .unwrap();

        let meta = fs.meta("test.txt").await.unwrap();
        assert_eq!(meta.name, "test.txt");
        assert_eq!(meta.size, 7);
        assert_eq!(meta.perm, "rw");
    }

    #[tokio::test]
    async fn get_not_found() {
        let fs = memory_fs();
        let result = fs.get("nonexistent.txt").await;
        assert!(matches!(result, Err(Error::NotFound(_))));
    }

    #[tokio::test]
    async fn meta_not_found() {
        let fs = memory_fs();
        let result = fs.meta("nonexistent.txt").await;
        assert!(matches!(result, Err(Error::NotFound(_))));
    }

    #[tokio::test]
    async fn delete_existing_file() {
        let fs = memory_fs();

        fs.put(
            "test.txt",
            bytes_stream(b"data"),
            IncomingFileMeta::default(),
        )
        .await
        .unwrap();

        fs.delete("test.txt").await.unwrap();

        let result = fs.get("test.txt").await;
        assert!(matches!(result, Err(Error::NotFound(_))));
    }

    #[tokio::test]
    async fn delete_not_found() {
        let fs = memory_fs();
        let result = fs.delete("nonexistent.txt").await;
        assert!(matches!(result, Err(Error::NotFound(_))));
    }

    #[tokio::test]
    async fn put_overwrites_existing() {
        let fs = memory_fs();

        fs.put(
            "test.txt",
            bytes_stream(b"first"),
            IncomingFileMeta::default(),
        )
        .await
        .unwrap();
        fs.put(
            "test.txt",
            bytes_stream(b"second"),
            IncomingFileMeta::default(),
        )
        .await
        .unwrap();

        let (stream, meta) = fs.get("test.txt").await.unwrap();
        assert_eq!(meta.size, 6);

        let data = collect_stream(stream).await;
        assert_eq!(data, b"second");
    }

    #[tokio::test]
    async fn default_content_type() {
        let fs = memory_fs();

        fs.put(
            "test.bin",
            bytes_stream(b"data"),
            IncomingFileMeta::default(),
        )
        .await
        .unwrap();

        let meta = fs.meta("test.bin").await.unwrap();
        assert_eq!(meta.content_type, "application/octet-stream");
    }
}
