use async_trait::async_trait;
use bytes::Bytes;
use futures::StreamExt as _;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod layer;

#[cfg(feature = "embed")]
pub mod embed;

#[cfg(feature = "opendal")]
pub mod opendal;

mod utils;

#[derive(Error, Debug)]
pub enum Error {
    #[error("File not found: {0}")]
    NotFound(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Permission denied: {0}")]
    PermissionDenied(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(any(
    not(target_arch = "wasm32"),
    all(target_arch = "wasm32", feature = "unsafe")
))]
pub type Stream = futures::stream::BoxStream<'static, std::result::Result<Bytes, std::io::Error>>;

#[cfg(all(target_arch = "wasm32", not(feature = "unsafe")))]
pub type Stream =
    futures::stream::LocalBoxStream<'static, std::result::Result<Bytes, std::io::Error>>;

#[cfg(not(target_arch = "wasm32"))]
pub trait StreamExt {
    fn into_boxed(self) -> Stream
    where
        Self: Sized
            + futures::Stream<Item = std::result::Result<Bytes, std::io::Error>>
            + Send
            + 'static,
    {
        self.boxed()
    }
}

#[cfg(all(target_arch = "wasm32", feature = "unsafe"))]
pub trait StreamExt {
    fn into_boxed(self) -> Stream
    where
        Self: Sized + futures::Stream<Item = std::result::Result<Bytes, std::io::Error>> + 'static,
    {
        let local_stream = self.boxed_local();

        // SAFETY: Only safe on single-threaded WASM environments
        unsafe { std::mem::transmute(local_stream) }
    }
}

#[cfg(all(target_arch = "wasm32", not(feature = "unsafe")))]
pub trait StreamExt {
    fn into_boxed(self) -> Stream
    where
        Self: Sized + futures::Stream<Item = std::result::Result<Bytes, std::io::Error>> + 'static,
    {
        self.boxed_local()
    }
}

impl<S> StreamExt for S where S: futures::Stream {}

#[allow(async_fn_in_trait)]
// #[async_trait(?Send)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait ReadOnlyFilesystem: Send + Sync {
    async fn list(&self) -> Result<Vec<FileMeta>>;
    async fn get(&self, path: &str) -> Result<(Stream, FileMeta)>;
    async fn meta(&self, path: &str) -> Result<FileMeta>;
}

#[allow(async_fn_in_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait WritableFilesystem: Send + Sync {
    async fn put(&self, path: &str, data: Stream, meta: IncomingFileMeta) -> Result<FileMeta>;
    async fn delete(&self, path: &str) -> Result<()>;
}

pub trait ReadWriteFilesystem: ReadOnlyFilesystem + WritableFilesystem {}
impl<T: ReadOnlyFilesystem + WritableFilesystem> ReadWriteFilesystem for T {}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMeta {
    pub name: String,
    pub created: u64,
    pub perm: String,
    pub content_type: String,
    pub last_modified: u64,
    pub size: u64,
}

#[cfg(feature = "http")]
impl TryFrom<FileMeta> for http::HeaderMap {
    type Error = http::header::InvalidHeaderValue;

    fn try_from(value: FileMeta) -> std::result::Result<Self, Self::Error> {
        let mut headers = http::HeaderMap::new();

        headers.insert(http::header::CONTENT_TYPE, value.content_type.parse()?);
        headers.insert(
            http::header::CONTENT_LENGTH,
            value.size.to_string().parse()?,
        );
        headers.insert("X-Content-Length", value.size.to_string().parse()?);
        headers.insert("X-Created", value.created.to_string().parse()?);
        headers.insert("X-Last-Modified", value.last_modified.to_string().parse()?);
        headers.insert("X-Permission", value.perm.as_str().parse()?);

        Ok(headers)
    }
}

#[derive(Debug, Clone, Default)]
pub struct IncomingFileMeta {
    pub created: Option<u64>,
    pub perm: Option<String>,
    pub content_type: Option<String>,
    pub last_modified: Option<u64>,
    pub size: Option<u64>,
}

#[cfg(feature = "http")]
impl TryFrom<http::HeaderMap> for IncomingFileMeta {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: http::HeaderMap) -> std::result::Result<Self, Self::Error> {
        use std::str::FromStr;

        fn get_header<T: FromStr>(
            headers: &http::HeaderMap,
            name: impl http::header::AsHeaderName,
        ) -> std::result::Result<Option<T>, Box<dyn std::error::Error>>
        where
            T::Err: std::error::Error + 'static,
        {
            headers
                .get(name)
                .map(|v| Ok(v.to_str()?.parse()?))
                .transpose()
        }

        Ok(IncomingFileMeta {
            created: get_header(&value, "x-created")?,
            content_type: get_header(&value, http::header::CONTENT_TYPE)?,
            ..Default::default()
        })
    }
}

#[cfg(feature = "axum")]
impl From<Error> for axum::http::StatusCode {
    fn from(value: Error) -> axum::http::StatusCode {
        match value {
            Error::NotFound(..) => axum::http::StatusCode::NOT_FOUND,
            Error::PermissionDenied(..) => axum::http::StatusCode::FORBIDDEN,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[cfg(feature = "axum")]
impl axum::response::IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        axum::http::StatusCode::from(self).into_response()
    }
}

#[cfg(test)]
mod tests {
    #[allow(dead_code)]
    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    #[cfg(all(not(target_arch = "wasm32"), feature = "opendal"))]
    fn opendal_filesystem_is_send_sync() {
        assert_send_sync::<crate::fs::opendal::Filesystem>();
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn layer_filesystem_is_send_sync() {
        assert_send_sync::<crate::fs::layer::Filesystem>();
    }

    #[test]
    #[cfg(all(not(target_arch = "wasm32"), feature = "embed"))]
    fn embed_filesystem_is_send_sync() {
        use rust_embed::Embed;

        #[derive(Embed)]
        #[folder = "src"]
        struct TestEmbed;

        assert_send_sync::<crate::fs::embed::Filesystem<TestEmbed>>();
    }
}

#[cfg(all(test, feature = "http"))]
mod http_tests {
    use super::*;

    #[test]
    fn file_meta_to_header_map() {
        let meta = FileMeta {
            name: "test.txt".to_string(),
            created: 1000000,
            perm: "rw".to_string(),
            content_type: "text/plain".to_string(),
            last_modified: 2000000,
            size: 42,
        };

        let headers: http::HeaderMap = meta.try_into().unwrap();

        assert_eq!(
            headers.get(http::header::CONTENT_TYPE).unwrap(),
            "text/plain"
        );
        assert_eq!(headers.get(http::header::CONTENT_LENGTH).unwrap(), "42");
        assert_eq!(headers.get("X-Content-Length").unwrap(), "42");
        assert_eq!(headers.get("X-Created").unwrap(), "1000000");
        assert_eq!(headers.get("X-Last-Modified").unwrap(), "2000000");
        assert_eq!(headers.get("X-Permission").unwrap(), "rw");
    }

    #[test]
    fn file_meta_to_header_map_invalid_content_type() {
        let meta = FileMeta {
            name: "test.txt".to_string(),
            created: 1000000,
            perm: "rw".to_string(),
            content_type: "invalid\x00header".to_string(),
            last_modified: 2000000,
            size: 42,
        };

        let result: std::result::Result<http::HeaderMap, _> = meta.try_into();
        assert!(result.is_err());
    }

    #[test]
    fn header_map_to_incoming_file_meta() {
        let mut headers = http::HeaderMap::new();
        headers.insert(
            http::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        headers.insert("x-created", "1234".parse().unwrap());

        let meta: IncomingFileMeta = headers.try_into().unwrap();

        assert_eq!(meta.content_type, Some("application/json".to_string()));
        assert_eq!(meta.created, Some(1234));
        assert_eq!(meta.perm, None);
        assert_eq!(meta.last_modified, None);
        assert_eq!(meta.size, None);
    }

    #[test]
    fn header_map_to_incoming_file_meta_empty() {
        let headers = http::HeaderMap::new();

        let meta: IncomingFileMeta = headers.try_into().unwrap();

        assert_eq!(meta.content_type, None);
        assert_eq!(meta.created, None);
        assert_eq!(meta.perm, None);
        assert_eq!(meta.last_modified, None);
        assert_eq!(meta.size, None);
    }

    #[test]
    fn header_map_to_incoming_file_meta_invalid_created() {
        let mut headers = http::HeaderMap::new();
        headers.insert("x-created", "not-a-number".parse().unwrap());

        let result: std::result::Result<IncomingFileMeta, _> = headers.try_into();
        assert!(result.is_err());
    }
}
