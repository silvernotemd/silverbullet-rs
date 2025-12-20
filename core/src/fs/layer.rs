use async_trait::async_trait;

use crate::fs::*;

pub struct LayerFS {
    layers: Vec<Box<dyn ReadOnlyFilesystem>>,
    root: Box<dyn ReadWriteFilesystem>,
}

impl LayerFS {
    pub fn builder<R>(root: R) -> LayerFSBuilder
    where
        R: ReadWriteFilesystem + 'static,
    {
        LayerFSBuilder {
            layers: Vec::new(),
            root: Box::new(root),
        }
    }
}

pub struct LayerFSBuilder {
    layers: Vec<Box<dyn ReadOnlyFilesystem>>,
    root: Box<dyn ReadWriteFilesystem>,
}

impl LayerFSBuilder {
    pub fn new<R>(root: R) -> Self
    where
        R: ReadWriteFilesystem + 'static,
    {
        Self {
            layers: Vec::new(),
            root: Box::new(root),
        }
    }

    pub fn layer<L>(mut self, layer: L) -> Self
    where
        L: ReadOnlyFilesystem + 'static,
    {
        self.layers.push(Box::new(layer));
        self
    }

    pub fn build(self) -> LayerFS {
        LayerFS {
            layers: self.layers,
            root: self.root,
        }
    }
}

#[async_trait(?Send)]
impl ReadOnlyFilesystem for LayerFS {
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

        Ok(all_files.into_values().collect())
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

#[async_trait(?Send)]
impl WritableFilesystem for LayerFS {
    async fn put(&self, path: &str, data: Stream) -> Result<FileMeta> {
        self.root.put(path, data).await
    }

    async fn delete(&self, path: &str) -> Result<()> {
        self.root.delete(path).await
    }
}
