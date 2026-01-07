use async_trait::async_trait;
use bytes::Bytes;
use futures::stream;
use rust_embed::Embed;
use web_time::{SystemTime, UNIX_EPOCH};

use crate::fs::*;

pub struct Filesystem<E: Embed> {
    embed: std::marker::PhantomData<E>,
}

impl<E: Embed> Default for Filesystem<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: Embed> Filesystem<E> {
    pub fn new() -> Self {
        Self {
            embed: std::marker::PhantomData,
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl<E: Embed + Send + Sync> ReadOnlyFilesystem for Filesystem<E> {
    async fn list(&self) -> Result<Vec<FileMeta>> {
        Ok(E::iter()
            .filter_map(|path| E::get(&path).map(|file| (path.as_ref(), file).into()))
            .collect())
    }

    async fn get(&self, path: &str) -> Result<(Stream, FileMeta)> {
        E::get(path)
            .map(|file| {
                let bytes = match file.data.clone() {
                    std::borrow::Cow::Borrowed(slice) => Bytes::from_static(slice),
                    std::borrow::Cow::Owned(vec) => Bytes::from(vec),
                };

                let stream = stream::once(std::future::ready(Ok::<Bytes, std::io::Error>(bytes)));

                use crate::fs::StreamExt;

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
                .unwrap_or_else(|| {
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0)
                }),
            perm: "ro".to_string(), // Default permission for embedded files
            content_type: file.metadata.mimetype().to_string(),
            last_modified: file
                .metadata
                .last_modified()
                .map(|s| s * 1000)
                .unwrap_or_else(|| {
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0)
                }),
            size: file.data.len() as u64,
        }
    }
}
