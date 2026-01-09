pub mod error;
use axum::extract::FromRef;
use axum::{Router, routing};
pub use error::*;

use crate::client;
use crate::server::fs::FilesystemProvider;

pub mod fs;
pub mod routes;

pub fn router<S>() -> Router<S>
where
    S: FilesystemProvider + Clone + Send + Sync + 'static,
    client::Config: FromRef<S>,
{
    Router::<S>::new()
        .nest("/.fs", fs::router())
        .route("/.shell", routing::post(routes::shell))
        .route("/.ping", routing::get(routes::ping))
        .route("/.logs", routing::post(routes::log))
        .route("/.config", routing::get(routes::config))
        .route(
            "/.client/manifest.json",
            routing::get(routes::client_manifest),
        )
}
