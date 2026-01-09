use axum::body::Body;
use axum::response::AppendHeaders;
use axum::{Json, Router};
use axum::{
    extract::{FromRequestParts, Path},
    response::IntoResponse,
    response::Response,
    routing,
};
use futures::TryStreamExt;
use http::request::Parts;
use http::{HeaderMap, StatusCode};

use crate::fs::{
    self, FileMeta, IncomingFileMeta, ReadOnlyFilesystem, ReadWriteFilesystem, Stream, StreamExt,
};
use crate::server::error::Error;

pub trait FilesystemProvider {
    type Fs: ReadWriteFilesystem;

    fn create_fs(&self, parts: &mut Parts) -> Result<Self::Fs, Error>;
}

pub struct Filesystem<F>(pub F);

impl<S> FromRequestParts<S> for Filesystem<S::Fs>
where
    S: FilesystemProvider + Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Ok(Filesystem(
            state.create_fs(parts).map_err(|err| err.into_response())?,
        ))
    }
}

pub fn router<S>() -> Router<S>
where
    S: FilesystemProvider + Clone + Send + Sync + 'static,
{
    Router::<S>::new()
        .route("/.fs", routing::get(list))
        .route("/.fs/{*path}", routing::get(get).put(put).options(options))
}

#[cfg_attr(feature = "cloudflare", worker::send)]
pub async fn list<F>(Filesystem(fs): Filesystem<F>) -> Result<impl IntoResponse, fs::Error>
where
    F: ReadOnlyFilesystem,
{
    let files = fs.list().await.map(|mut files| {
        files.sort_by(|a, b| a.name.cmp(&b.name));
    })?;

    Ok(Json(files))
}

#[cfg_attr(feature = "cloudflare", worker::send)]
pub async fn get<F>(
    Filesystem(fs): Filesystem<F>,
    Path(path): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, Response>
where
    F: ReadOnlyFilesystem,
{
    let meta: FileMeta;
    let body;

    if headers.contains_key("X-Get-Meta") {
        meta = fs.meta(&path).await?;
        body = Body::empty();
    } else {
        let stream;

        (stream, meta) = fs.get(&path).await?;

        body = Body::from_stream(stream);
    }

    Ok((HeaderMap::try_from(meta).map_err(Error::from)?, body))
}

#[cfg_attr(feature = "cloudflare", worker::send)]
pub async fn put<F>(
    Filesystem(fs): Filesystem<F>,
    Path(path): Path<String>,
    incoming_meta: IncomingFileMeta,
    body: Body,
) -> Result<impl IntoResponse, Response>
where
    F: ReadWriteFilesystem,
{
    let stream: Stream = body
        .into_data_stream()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
        .into_boxed();

    let meta = fs.put(&path, stream, incoming_meta).await?;

    Ok((
        HeaderMap::try_from(meta.clone()).map_err(Error::from)?,
        AppendHeaders([("Cache-Control", "no-cache")]),
        Json(meta),
    ))
}

pub async fn options() -> impl IntoResponse {
    ([("Allow", "GET, PUT, DELETE, OPTIONS")], StatusCode::OK)
}
