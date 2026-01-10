use async_trait::async_trait;

use crate::fs::*;

pub struct Filesystem {
    layers: Vec<Box<dyn ReadOnlyFilesystem + Send + Sync>>,
    root: Box<dyn ReadWriteFilesystem + Send + Sync>,
}

impl Filesystem {
    pub fn builder<R>(root: R) -> Builder
    where
        R: ReadWriteFilesystem + Send + Sync + 'static,
    {
        Builder::new(root)
    }
}

pub struct Builder {
    layers: Vec<Box<dyn ReadOnlyFilesystem + Send + Sync>>,
    root: Box<dyn ReadWriteFilesystem + Send + Sync>,
}

impl Builder {
    pub fn new<R>(root: R) -> Self
    where
        R: ReadWriteFilesystem + Send + Sync + 'static,
    {
        Self {
            layers: Vec::new(),
            root: Box::new(root),
        }
    }

    #[must_use]
    pub fn layer<L>(mut self, layer: L) -> Self
    where
        L: ReadOnlyFilesystem + Send + Sync + 'static,
    {
        self.layers.push(Box::new(layer));
        self
    }

    #[must_use]
    pub fn build(self) -> Filesystem {
        Filesystem {
            layers: self.layers,
            root: self.root,
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl ReadOnlyFilesystem for Filesystem {
    async fn list(&self) -> Result<Vec<FileMeta>> {
        let mut all_files = std::collections::HashMap::new();

        // Start with root (lowest priority)
        if let Ok(files) = self.root.list().await {
            for file in files {
                all_files.insert(file.name.clone(), file);
            }
        }

        // Apply layers in reverse order (last layer = highest priority)
        for layer in self.layers.iter().rev() {
            if let Ok(files) = layer.list().await {
                for file in files {
                    all_files.insert(file.name.clone(), file);
                }
            }
        }

        let mut files: Vec<_> = all_files.into_values().collect();
        files.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(files)
    }

    async fn get(&self, path: &str) -> Result<(Stream, FileMeta)> {
        // Try each layer first (last layer = highest priority)
        for layer in self.layers.iter().rev() {
            if let Ok(result) = layer.get(path).await {
                return Ok(result);
            }
        }

        // Fall back to root
        self.root.get(path).await
    }

    async fn meta(&self, path: &str) -> Result<FileMeta> {
        // Try each layer first (last layer = highest priority)
        for layer in self.layers.iter().rev() {
            if let Ok(meta) = layer.meta(path).await {
                return Ok(meta);
            }
        }

        // Fall back to root
        self.root.meta(path).await
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl WritableFilesystem for Filesystem {
    async fn put(&self, path: &str, data: Stream, meta: IncomingFileMeta) -> Result<FileMeta> {
        self.root.put(path, data, meta).await
    }

    async fn delete(&self, path: &str) -> Result<()> {
        self.root.delete(path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use futures::stream;
    use std::collections::HashMap;
    use std::sync::RwLock;

    /// A simple in-memory filesystem for testing
    struct MemoryFs {
        files: RwLock<HashMap<String, (Bytes, FileMeta)>>,
    }

    impl MemoryFs {
        fn new() -> Self {
            Self {
                files: RwLock::new(HashMap::new()),
            }
        }

        fn with_file(self, name: &str, content: &[u8]) -> Self {
            self.files.write().unwrap().insert(
                name.to_string(),
                (
                    Bytes::copy_from_slice(content),
                    FileMeta {
                        name: name.to_string(),
                        created: 0,
                        perm: "rw".to_string(),
                        content_type: "text/plain".to_string(),
                        last_modified: 0,
                        size: content.len() as u64,
                    },
                ),
            );
            self
        }
    }

    #[async_trait]
    impl ReadOnlyFilesystem for MemoryFs {
        async fn list(&self) -> Result<Vec<FileMeta>> {
            Ok(self
                .files
                .read()
                .unwrap()
                .values()
                .map(|(_, meta)| meta.clone())
                .collect())
        }

        async fn get(&self, path: &str) -> Result<(Stream, FileMeta)> {
            let files = self.files.read().unwrap();
            let (data, meta) = files
                .get(path)
                .ok_or_else(|| Error::NotFound(path.into()))?;
            let data = data.clone();
            let meta = meta.clone();
            Ok((stream::once(async move { Ok(data) }).boxed(), meta))
        }

        async fn meta(&self, path: &str) -> Result<FileMeta> {
            self.files
                .read()
                .unwrap()
                .get(path)
                .map(|(_, meta)| meta.clone())
                .ok_or_else(|| Error::NotFound(path.into()))
        }
    }

    #[async_trait]
    impl WritableFilesystem for MemoryFs {
        async fn put(&self, path: &str, data: Stream, meta: IncomingFileMeta) -> Result<FileMeta> {
            use futures::TryStreamExt;
            let bytes: Vec<u8> = data
                .try_fold(Vec::new(), |mut acc, chunk| async move {
                    acc.extend_from_slice(&chunk);
                    Ok(acc)
                })
                .await?;

            let file_meta = FileMeta {
                name: path.to_string(),
                created: meta.created.unwrap_or(0),
                perm: meta.perm.unwrap_or_else(|| "rw".to_string()),
                content_type: meta
                    .content_type
                    .unwrap_or_else(|| "text/plain".to_string()),
                last_modified: meta.last_modified.unwrap_or(0),
                size: bytes.len() as u64,
            };

            self.files
                .write()
                .unwrap()
                .insert(path.to_string(), (Bytes::from(bytes), file_meta.clone()));

            Ok(file_meta)
        }

        async fn delete(&self, path: &str) -> Result<()> {
            self.files
                .write()
                .unwrap()
                .remove(path)
                .ok_or_else(|| Error::NotFound(path.into()))?;
            Ok(())
        }
    }

    #[tokio::test]
    async fn get_returns_from_root_when_no_layers() {
        let fs =
            Filesystem::builder(MemoryFs::new().with_file("test.txt", b"root content")).build();

        let (stream, meta) = fs.get("test.txt").await.unwrap();
        let content = read_stream(stream).await;

        assert_eq!(meta.name, "test.txt");
        assert_eq!(content, b"root content");
    }

    #[tokio::test]
    async fn get_returns_from_layer_over_root() {
        let root = MemoryFs::new().with_file("test.txt", b"root content");
        let layer = MemoryFs::new().with_file("test.txt", b"layer content");

        let fs = Filesystem::builder(root).layer(layer).build();

        let (stream, _) = fs.get("test.txt").await.unwrap();
        let content = read_stream(stream).await;

        assert_eq!(content, b"layer content");
    }

    #[tokio::test]
    async fn get_last_layer_has_highest_priority() {
        let root = MemoryFs::new().with_file("test.txt", b"root");
        let layer1 = MemoryFs::new().with_file("test.txt", b"layer1");
        let layer2 = MemoryFs::new().with_file("test.txt", b"layer2");

        let fs = Filesystem::builder(root)
            .layer(layer1)
            .layer(layer2)
            .build();

        let (stream, _) = fs.get("test.txt").await.unwrap();
        let content = read_stream(stream).await;

        assert_eq!(content, b"layer2");
    }

    #[tokio::test]
    async fn get_falls_through_to_root_when_not_in_layer() {
        let root = MemoryFs::new().with_file("root-only.txt", b"root content");
        let layer = MemoryFs::new().with_file("layer-only.txt", b"layer content");

        let fs = Filesystem::builder(root).layer(layer).build();

        let (stream, _) = fs.get("root-only.txt").await.unwrap();
        assert_eq!(read_stream(stream).await, b"root content");

        let (stream, _) = fs.get("layer-only.txt").await.unwrap();
        assert_eq!(read_stream(stream).await, b"layer content");
    }

    #[tokio::test]
    async fn list_merges_files_from_all_sources() {
        let root = MemoryFs::new().with_file("a.txt", b"");
        let layer = MemoryFs::new().with_file("b.txt", b"");

        let fs = Filesystem::builder(root).layer(layer).build();

        let files = fs.list().await.unwrap();
        let names: Vec<_> = files.iter().map(|f| f.name.as_str()).collect();

        assert_eq!(names, vec!["a.txt", "b.txt"]);
    }

    #[tokio::test]
    async fn list_returns_sorted_results() {
        let root = MemoryFs::new()
            .with_file("zebra.txt", b"")
            .with_file("alpha.txt", b"")
            .with_file("mango.txt", b"");

        let fs = Filesystem::builder(root).build();

        let files = fs.list().await.unwrap();
        let names: Vec<_> = files.iter().map(|f| f.name.as_str()).collect();

        assert_eq!(names, vec!["alpha.txt", "mango.txt", "zebra.txt"]);
    }

    #[tokio::test]
    async fn list_layer_file_shadows_root_file() {
        let root = MemoryFs::new().with_file("test.txt", b"small");
        let layer = MemoryFs::new().with_file("test.txt", b"much larger content");

        let fs = Filesystem::builder(root).layer(layer).build();

        let files = fs.list().await.unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].size, 19); // layer's size
    }

    #[tokio::test]
    async fn write_goes_to_root() {
        let root = MemoryFs::new();
        let layer = MemoryFs::new();

        let fs = Filesystem::builder(root).layer(layer).build();

        let data = stream::once(async { Ok(Bytes::from("new content")) }).boxed();
        fs.put("new.txt", data, IncomingFileMeta::default())
            .await
            .unwrap();

        // Should be readable (from root)
        let (stream, _) = fs.get("new.txt").await.unwrap();
        assert_eq!(read_stream(stream).await, b"new content");
    }

    #[tokio::test]
    async fn delete_removes_from_root() {
        let root = MemoryFs::new().with_file("test.txt", b"content");
        let fs = Filesystem::builder(root).build();

        fs.delete("test.txt").await.unwrap();

        assert!(fs.get("test.txt").await.is_err());
    }

    #[tokio::test]
    async fn delete_does_not_affect_layer_file() {
        let root = MemoryFs::new().with_file("test.txt", b"root");
        let layer = MemoryFs::new().with_file("test.txt", b"layer");

        let fs = Filesystem::builder(root).layer(layer).build();

        // Delete from root - layer file should still be visible
        fs.delete("test.txt").await.unwrap();

        let (stream, _) = fs.get("test.txt").await.unwrap();
        assert_eq!(read_stream(stream).await, b"layer");
    }

    async fn read_stream(stream: Stream) -> Vec<u8> {
        use futures::TryStreamExt;
        stream
            .try_fold(Vec::new(), |mut acc, chunk| async move {
                acc.extend_from_slice(&chunk);
                Ok(acc)
            })
            .await
            .unwrap()
    }
}
