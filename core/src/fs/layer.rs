use async_trait::async_trait;

use crate::fs::*;

// #[cfg(target_arch = "wasm32")]
// pub struct Filesystem {
//     layers: Vec<Box<dyn ReadOnlyFilesystem>>,
//     root: Box<dyn ReadWriteFilesystem>,
// }

// #[cfg(not(target_arch = "wasm32"))]
pub struct Filesystem {
    layers: Vec<Box<dyn ReadOnlyFilesystem + Send + Sync>>,
    root: Box<dyn ReadWriteFilesystem + Send + Sync>,
}

// #[cfg(target_arch = "wasm32")]
// impl Filesystem {
//     pub fn builder<R>(root: R) -> Builder
//     where
//         R: ReadWriteFilesystem + 'static,
//     {
//         Builder {
//             layers: Vec::new(),
//             root: Box::new(root),
//         }
//     }
// }

// #[cfg(not(target_arch = "wasm32"))]
impl Filesystem {
    pub fn builder<R>(root: R) -> Builder
    where
        R: ReadWriteFilesystem + Send + Sync + 'static,
    {
        Builder {
            layers: Vec::new(),
            root: Box::new(root),
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub struct Builder {
    layers: Vec<Box<dyn ReadOnlyFilesystem>>,
    root: Box<dyn ReadWriteFilesystem>,
}

#[cfg(not(target_arch = "wasm32"))]
pub struct Builder {
    layers: Vec<Box<dyn ReadOnlyFilesystem + Send + Sync>>,
    root: Box<dyn ReadWriteFilesystem + Send + Sync>,
}

#[cfg(target_arch = "wasm32")]
impl Builder {
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

    pub fn build(self) -> Filesystem {
        Filesystem {
            layers: self.layers,
            root: self.root,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
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

    pub fn layer<L>(mut self, layer: L) -> Self
    where
        L: ReadOnlyFilesystem + Send + Sync + 'static,
    {
        self.layers.push(Box::new(layer));
        self
    }

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
