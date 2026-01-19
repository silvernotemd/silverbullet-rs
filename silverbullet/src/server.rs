pub mod error;
pub use error::*;

pub mod routes;

use axum::{Router, extract::FromRef, routing};

use crate::client;

pub fn router<S>() -> Router<S>
where
    S: routes::fs::Provider
        + routes::shell::Provider
        + routes::proxy::Provider
        + routes::log::Provider
        + Clone
        + Send
        + Sync
        + 'static,
    client::Config: FromRef<S>,
{
    Router::<S>::new()
        .nest("/.fs", routes::fs::router())
        .route("/.shell", routing::post(routes::shell::shell))
        .route("/.proxy/{*url}", routing::any(routes::proxy::proxy))
        .route("/.ping", routing::get(routes::ping))
        .route("/.logs", routing::post(routes::log::log))
        .route("/.config", routing::get(routes::config))
        .route(
            "/.client/manifest.json",
            routing::get(routes::client_manifest),
        )
}
