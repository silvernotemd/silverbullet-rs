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

#[cfg(target_arch = "wasm32")]
pub type Stream =
    futures::stream::LocalBoxStream<'static, std::result::Result<Bytes, std::io::Error>>;

#[cfg(target_arch = "wasm32")]
fn box_stream<S>(stream: S) -> Stream
where
    S: futures::Stream<Item = std::result::Result<Bytes, std::io::Error>> + 'static,
{
    stream.boxed_local()
}

#[cfg(target_arch = "wasm32")]
pub trait StreamExt {
    fn into_boxed(self) -> Stream
    where
        Self: Sized + futures::Stream<Item = std::result::Result<Bytes, std::io::Error>> + 'static,
    {
        box_stream(self)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub type Stream = futures::stream::BoxStream<'static, std::result::Result<Bytes, std::io::Error>>;

#[cfg(not(target_arch = "wasm32"))]
fn box_stream<S>(stream: S) -> Stream
where
    S: futures::Stream<Item = std::result::Result<Bytes, std::io::Error>> + Send + 'static,
{
    stream.boxed()
}

#[cfg(not(target_arch = "wasm32"))]
pub trait StreamExt {
    fn into_boxed(self) -> Stream
    where
        Self: Sized
            + futures::Stream<Item = std::result::Result<Bytes, std::io::Error>>
            + Send
            + 'static,
    {
        box_stream(self)
    }
}

impl<S> StreamExt for S where S: futures::Stream {}

#[allow(async_fn_in_trait)]
#[async_trait(?Send)]
pub trait ReadOnlyFilesystem {
    async fn list(&self) -> Result<Vec<FileMeta>>;
    async fn get(&self, path: &str) -> Result<(Stream, FileMeta)>;
    async fn meta(&self, path: &str) -> Result<FileMeta>;
}

#[allow(async_fn_in_trait)]
#[async_trait(?Send)]
pub trait WritableFilesystem {
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

#[derive(Debug, Clone)]
pub struct IncomingFileMeta {
    pub created: Option<u64>,
    pub perm: Option<String>,
    pub content_type: Option<String>,
    pub last_modified: Option<u64>,
    pub size: Option<u64>,
}
