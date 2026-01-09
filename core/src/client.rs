use serde::{Deserialize, Serialize};

#[cfg(feature = "tracing")]
mod tracing;

#[cfg(feature = "tracing")]
pub use tracing::*;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub space_folder_path: String,
    pub index_page: String,
    pub read_only: bool,
    pub log_push: bool,
    pub enable_client_encryption: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Manifest {
    pub short_name: String,
    pub name: String,
    pub icons: Vec<ManifestIcon>,
    pub capture_links: String,
    pub start_url: String,
    pub display: String,
    pub display_override: Vec<String>,
    pub scope: String,
    pub theme_color: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ManifestIcon {
    pub src: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub sizes: String,
}

pub trait Logger {
    fn log(&self, client_ip: String, entries: Vec<LogEntry>);
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LogEntry {
    pub source: String,
    pub level: String,
    pub message: String,
    pub timestamp: i64,
}
