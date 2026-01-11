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

pub trait Provider {
    type Output: ReadWriteFilesystem;

    fn provide(&self, parts: &mut Parts) -> Result<Self::Output, Error>;
}

pub struct Filesystem<F>(pub F);

impl<S> FromRequestParts<S> for Filesystem<S::Output>
where
    S: Provider + Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Ok(Filesystem(
            state.provide(parts).map_err(|err| err.into_response())?,
        ))
    }
}

pub fn router<S>() -> Router<S>
where
    S: Provider + Clone + Send + Sync + 'static,
{
    Router::<S>::new().route("/", routing::get(list)).route(
        "/{*path}",
        routing::get(get).put(put).delete(delete).options(options),
    )
}

#[cfg_attr(feature = "cloudflare", worker::send)]
pub async fn list<F>(Filesystem(fs): Filesystem<F>) -> Result<impl IntoResponse, fs::Error>
where
    F: ReadOnlyFilesystem,
{
    let files = fs.list().await.map(|mut files| {
        files.sort_by(|a, b| a.name.cmp(&b.name));

        files
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
        .map_err(std::io::Error::other)
        .into_boxed();

    let meta = fs.put(&path, stream, incoming_meta).await?;

    Ok((
        HeaderMap::try_from(meta.clone()).map_err(Error::from)?,
        AppendHeaders([("Cache-Control", "no-cache")]),
        Json(meta),
    ))
}

#[cfg_attr(feature = "cloudflare", worker::send)]
pub async fn delete<F>(
    Filesystem(fs): Filesystem<F>,
    Path(path): Path<String>,
) -> Result<impl IntoResponse, fs::Error>
where
    F: ReadWriteFilesystem,
{
    fs.delete(&path).await?;

    // Returns 200 OK with body "OK" to match the original SilverBullet API.
    Ok("OK")
}

pub async fn options() -> impl IntoResponse {
    ([("Allow", "GET, PUT, DELETE, OPTIONS")], StatusCode::OK)
}
