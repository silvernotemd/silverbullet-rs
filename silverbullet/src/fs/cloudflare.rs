use std::collections::HashMap;

use async_trait::async_trait;
use bytes::Bytes;
use futures::StreamExt;
use worker::{Bucket, Data, FixedLengthStream, HttpMetadata, Include};

use crate::fs::*;

pub struct Filesystem {
    bucket: Bucket,
    prefix: String,
    allow_buffered_upload: bool,
}

// SAFETY: wasm32 is single-threaded, so Send + Sync is safe
unsafe impl Send for Filesystem {}
unsafe impl Sync for Filesystem {}

impl Filesystem {
    pub fn new(bucket: Bucket, prefix: String) -> Self {
        Self {
            bucket,
            prefix,
            allow_buffered_upload: false,
        }
    }

    /// Allow falling back to buffered uploads when size is not provided.
    /// This may cause memory issues with large files.
    #[allow(dead_code)]
    pub fn allow_buffered_upload(mut self, allow: bool) -> Self {
        self.allow_buffered_upload = allow;
        self
    }

    fn full_path(&self, path: &str) -> String {
        if self.prefix.is_empty() {
            path.to_string()
        } else {
            format!(
                "{}/{}",
                self.prefix.trim_end_matches('/'),
                path.trim_start_matches('/')
            )
        }
    }

    fn strip_prefix<'a>(&self, path: &'a str) -> &'a str {
        if self.prefix.is_empty() {
            path
        } else {
            let prefix_with_slash = format!("{}/", self.prefix.trim_end_matches('/'));
            path.strip_prefix(&prefix_with_slash).unwrap_or(path)
        }
    }
}

fn file_meta_from_r2_object(object: &worker::Object, name: &str) -> FileMeta {
    let now_millis = worker::Date::now().as_millis();

    let custom_metadata = object.custom_metadata().ok();
    let created = custom_metadata
        .as_ref()
        .and_then(|m| m.get("created"))
        .and_then(|s| s.parse().ok())
        .unwrap_or(now_millis);

    let content_type = object
        .http_metadata()
        .content_type
        .unwrap_or_else(|| "application/octet-stream".to_string());

    let last_modified = object.uploaded().as_millis();

    FileMeta {
        name: name.to_string(),
        created,
        perm: "rw".to_string(),
        content_type,
        last_modified,
        size: object.size(),
    }
}

#[async_trait(?Send)]
impl ReadOnlyFilesystem for Filesystem {
    async fn list(&self) -> Result<Vec<FileMeta>> {
        let mut all_objects = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let mut list_builder = self
                .bucket
                .list()
                .include(vec![Include::HttpMetadata, Include::CustomMetadata]);

            if !self.prefix.is_empty() {
                list_builder =
                    list_builder.prefix(format!("{}/", self.prefix.trim_end_matches('/')));
            }

            if let Some(ref c) = cursor {
                list_builder = list_builder.cursor(c.clone());
            }

            let objects = list_builder
                .execute()
                .await
                .map_err(|e| Error::Other(e.into()))?;

            for obj in objects.objects() {
                let name = self.strip_prefix(&obj.key()).to_string();
                all_objects.push(file_meta_from_r2_object(&obj, &name));
            }

            cursor = objects.cursor();
            if cursor.is_none() || !objects.truncated() {
                break;
            }
        }

        Ok(all_objects)
    }

    async fn get(&self, path: &str) -> Result<(Stream, FileMeta)> {
        let full_path = self.full_path(path);

        let object = self
            .bucket
            .get(&full_path)
            .execute()
            .await
            .map_err(|e| Error::Other(e.to_string().into()))?
            .ok_or_else(|| Error::NotFound(format!("Object not found: {}", path).into()))?;

        let meta = file_meta_from_r2_object(&object, path);

        let body = object
            .body()
            .ok_or_else(|| Error::Other("Object has no body".into()))?;

        let byte_stream = body
            .stream()
            .map_err(|e| Error::Other(e.to_string().into()))?;

        let stream = byte_stream.map(|result| {
            result
                .map(Bytes::from)
                .map_err(|e| std::io::Error::other(e.to_string()))
        });

        use crate::fs::StreamExt;

        Ok((stream.into_boxed(), meta))
    }

    async fn meta(&self, path: &str) -> Result<FileMeta> {
        let full_path = self.full_path(path);

        let object = self
            .bucket
            .head(&full_path)
            .await
            .map_err(|e| Error::Other(e.to_string().into()))?
            .ok_or_else(|| Error::NotFound(format!("Object not found: {}", path).into()))?;

        Ok(file_meta_from_r2_object(&object, path))
    }
}

#[async_trait(?Send)]
impl WritableFilesystem for Filesystem {
    async fn put(&self, path: &str, mut data: Stream, meta: IncomingFileMeta) -> Result<FileMeta> {
        let full_path = self.full_path(path);

        let http_metadata = HttpMetadata {
            content_type: meta.content_type.clone(),
            ..Default::default()
        };

        let mut custom_metadata = HashMap::<String, String>::new();
        if let Some(created) = meta.created {
            custom_metadata.insert("created".to_string(), created.to_string());
        }

        let r2_data = match meta.size {
            Some(size) => {
                // Stream directly to R2 without buffering
                let byte_stream = data.map(|result| {
                    result
                        .map(|bytes| bytes.to_vec())
                        .map_err(|e| worker::Error::RustError(e.to_string()))
                });
                Data::Stream(FixedLengthStream::wrap(byte_stream, size))
            }
            None if self.allow_buffered_upload => {
                // Fall back to buffering the entire file in memory
                let mut buffer = Vec::new();
                while let Some(chunk) = data.next().await {
                    buffer.extend_from_slice(&chunk?);
                }
                Data::Bytes(buffer)
            }
            None => {
                return Err(Error::Other(
                    "Size must be provided for streaming uploads".into(),
                ));
            }
        };

        let object = self
            .bucket
            .put(&full_path, r2_data)
            .http_metadata(http_metadata)
            .custom_metadata(custom_metadata)
            .execute()
            .await
            .map_err(|e| Error::Other(e.to_string().into()))?;

        Ok(file_meta_from_r2_object(&object, path))
    }

    async fn delete(&self, path: &str) -> Result<()> {
        let full_path = self.full_path(path);

        // Check if file exists first (required by the SilverBullet API)
        self.bucket
            .head(&full_path)
            .await
            .map_err(|e| Error::Other(e.to_string().into()))?
            .ok_or_else(|| Error::NotFound(format!("Object not found: {}", path).into()))?;

        self.bucket
            .delete(&full_path)
            .await
            .map_err(|e| Error::Other(e.to_string().into()))?;

        Ok(())
    }
}
