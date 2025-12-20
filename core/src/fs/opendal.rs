use ::opendal::Operator;
use async_trait::async_trait;
use futures::StreamExt;

use crate::fs::*;

pub struct Filesystem {
    operator: Operator,
}

impl Filesystem {
    pub fn new(operator: Operator) -> Self {
        Self { operator }
    }
}

#[async_trait(?Send)]
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

        Ok((box_stream(stream), (path, stat).into()))
    }

    async fn meta(&self, path: &str) -> Result<FileMeta> {
        let stat = self.operator.stat(path).await?;

        Ok((path, stat).into())
    }
}

#[async_trait(?Send)]
impl WritableFilesystem for Filesystem {
    async fn put(&self, path: &str, mut data: Stream) -> Result<FileMeta> {
        let mut writer = self.operator.writer(path).await?;

        // Stream the data to OpenDAL
        while let Some(chunk) = data.next().await {
            let bytes = chunk?;
            writer.write(bytes).await?;
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
                .unwrap_or_else(crate::fs::now::unix_timestamp),
            perm: "rw".to_string(), // Default to read-write for now
            content_type: metadata
                .content_type()
                .or_else(|| mime_guess::from_path(path).first_raw()) // TODO: this can probably be removed when using the mime guesser layer
                .unwrap_or("application/octet-stream")
                .to_string(),

            last_modified: metadata
                .last_modified()
                .map(|lm| lm.into_inner().as_second().unsigned_abs())
                .unwrap_or_else(crate::fs::now::unix_timestamp),
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
