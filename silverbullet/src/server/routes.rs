pub mod fs;
pub mod log;
pub mod proxy;
pub mod shell;

use axum::{extract::State, response::IntoResponse};

use crate::client;

#[cfg_attr(feature = "debug", axum::debug_handler)]
pub async fn config(State(config): State<client::Config>) -> impl IntoResponse {
    ([("Cache-Control", "no-cache")], axum::Json(config))
}

#[cfg_attr(feature = "debug", axum::debug_handler)]
pub async fn client_manifest() -> impl IntoResponse {
    let host_prefix_url: Option<String> = None;

    let client_manifest = client::Manifest {
        short_name: "space name".to_string(),
        name: "space name".to_string(),
        icons: vec![client::ManifestIcon {
            src: host_prefix_url
                .clone()
                .map_or("/.client/logo-dock.png".to_string(), |u| {
                    format!("{}/.client/logo-dock.png", u)
                }),
            sizes: "512x512".to_string(),
            type_: "image/png".to_string(),
        }],
        capture_links: "new-client".to_string(),
        start_url: host_prefix_url
            .clone()
            .map_or("/#boot".to_string(), |u| format!("{}/#boot", u)),
        display: "standalone".to_string(),
        display_override: vec!["window-controls-overlay".to_string()],
        scope: host_prefix_url
            .clone()
            .map_or("/".to_string(), |u| format!("{}/", u)),
        theme_color: "#e1e1e1".to_string(),
        description: "description".to_string(),
    };

    axum::Json(client_manifest)
}

#[cfg_attr(feature = "debug", axum::debug_handler)]
pub async fn ping() -> impl IntoResponse {
    ([("Cache-Control", "no-cache"), ("X-Space-Path", "")], "OK")
}
