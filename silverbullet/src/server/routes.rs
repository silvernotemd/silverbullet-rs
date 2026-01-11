pub mod fs;
pub mod shell;

// use axum::Json;
use axum::response::IntoResponse;
use http::HeaderMap;

use crate::client::Logger;
use crate::client::{self, TracingLogger};
// use crate::shell;

// pub async fn shell() -> Json<shell::Response> {
//     axum::Json(shell::Response {
//         code: 1,
//         stdout: "".to_string(),
//         stderr: "Not supported".to_string(),
//     })
// }

pub async fn log(
    headers: HeaderMap,
    axum::Json(entries): axum::Json<Vec<client::LogEntry>>,
) -> axum::http::StatusCode {
    let ip = headers
        .get("cf-connecting-ip")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("<unknown-ip>");

    let logger = TracingLogger::new();
    logger.log(ip.to_string(), entries);

    axum::http::StatusCode::OK
}

pub async fn config(
    axum::extract::State(config): axum::extract::State<client::Config>,
) -> impl IntoResponse {
    ([("Cache-Control", "no-cache")], axum::Json(config))
}

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

pub async fn ping() -> impl IntoResponse {
    ([("Cache-Control", "no-cache"), ("X-Space-Path", "")], "OK")
}
