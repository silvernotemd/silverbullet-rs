use std::borrow::Cow;

use async_trait::async_trait;
use bytes::Bytes;
use futures::stream;
use rust_embed::Embed;

use crate::fs::*;

pub struct Filesystem<E> {
    embed: std::marker::PhantomData<E>,
}

impl<E> Default for Filesystem<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E> Filesystem<E> {
    pub fn new() -> Self {
        Self {
            embed: std::marker::PhantomData,
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl<E> ReadOnlyFilesystem for Filesystem<E>
where
    E: Embed + Send + Sync,
{
    async fn list(&self) -> Result<Vec<FileMeta>> {
        Ok(E::iter()
            .filter_map(|path| E::get(&path).map(|file| (path.as_ref(), file).into()))
            .collect())
    }

    async fn get(&self, path: &str) -> Result<(Stream, FileMeta)> {
        E::get(path)
            .map(|file| {
                let bytes = match file.data.clone() {
                    Cow::Borrowed(slice) => Bytes::from_static(slice),
                    Cow::Owned(vec) => Bytes::from(vec),
                };

                let stream = stream::once(std::future::ready(Ok::<Bytes, std::io::Error>(bytes)));

                (stream.into_boxed(), (path, file).into())
            })
            .ok_or_else(|| Error::NotFound(format!("Embedded file not found: {}", path).into()))
    }

    async fn meta(&self, path: &str) -> Result<FileMeta> {
        E::get(path)
            .map(|file| (path, file).into())
            .ok_or_else(|| Error::NotFound(format!("Embedded file not found: {}", path).into()))
    }
}

impl From<(&str, rust_embed::EmbeddedFile)> for FileMeta {
    fn from((name, file): (&str, rust_embed::EmbeddedFile)) -> Self {
        FileMeta {
            name: name.to_string(),
            created: file
                .metadata
                .created()
                .map(|s| s * 1000)
                .unwrap_or_else(utils::now),
            perm: "ro".to_string(), // Default permission for embedded files
            content_type: file.metadata.mimetype().to_string(),
            last_modified: file
                .metadata
                .last_modified()
                .map(|s| s * 1000)
                .unwrap_or_else(utils::now),
            size: file.data.len() as u64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{StreamExt, executor::block_on};
    use rust_embed::Embed;

    // Embed a single known file for testing
    #[derive(Embed)]
    #[folder = "src/fs/"]
    struct TestEmbed;

    #[test]
    fn list_returns_embedded_files() {
        let fs: Filesystem<TestEmbed> = Filesystem::new();

        let files = block_on(fs.list()).unwrap();

        assert!(!files.is_empty());
        assert!(files.iter().any(|f| f.name == "embed.rs"));
        assert!(files.iter().all(|f| f.perm == "ro"));
    }

    #[test]
    fn get_returns_file_content_and_meta() {
        let fs: Filesystem<TestEmbed> = Filesystem::new();

        let (stream, meta) = block_on(fs.get("embed.rs")).unwrap();

        assert_eq!(meta.name, "embed.rs");
        assert_eq!(meta.perm, "ro");
        assert!(meta.size > 0);

        let bytes: Vec<_> = block_on(stream.collect());
        let content = String::from_utf8(bytes[0].as_ref().unwrap().to_vec()).unwrap();
        assert!(content.contains("pub struct Filesystem"));
    }

    #[test]
    fn get_nonexistent_file_returns_not_found() {
        let fs: Filesystem<TestEmbed> = Filesystem::new();

        let result = block_on(fs.get("nonexistent.rs"));

        assert!(matches!(result, Err(Error::NotFound(_))));
    }

    #[test]
    fn meta_returns_file_metadata() {
        let fs: Filesystem<TestEmbed> = Filesystem::new();

        let meta = block_on(fs.meta("embed.rs")).unwrap();

        assert_eq!(meta.name, "embed.rs");
        assert_eq!(meta.perm, "ro");
        assert!(meta.size > 0);
        assert_eq!(meta.content_type, "text/x-rust");
    }

    #[test]
    fn meta_nonexistent_file_returns_not_found() {
        let fs: Filesystem<TestEmbed> = Filesystem::new();

        let result = block_on(fs.meta("nonexistent.rs"));

        assert!(matches!(result, Err(Error::NotFound(_))));
    }

    #[test]
    fn filesystem_default_works() {
        let _fs: Filesystem<TestEmbed> = Filesystem::default();
    }
}
