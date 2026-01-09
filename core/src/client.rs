use serde::{Deserialize, Serialize};

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

#[cfg(feature = "tracing")]
pub struct TracingLogger;

#[cfg(feature = "tracing")]
impl Default for TracingLogger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "tracing")]
impl TracingLogger {
    pub fn new() -> Self {
        TracingLogger
    }
}

#[cfg(feature = "tracing")]
macro_rules! tracing_log_entry {
    ($level:expr, $entry:expr) => {
        tracing::event!(
            target: "client",
            $level,
            message = $entry.message,
            timestamp = $entry.timestamp,
        )
    };
}

#[cfg(feature = "tracing")]
impl Logger for TracingLogger {
    fn log(&self, _client_ip: String, entries: Vec<LogEntry>) {
        for entry in entries {
            match entry.level.to_lowercase().as_str() {
                "trace" => tracing_log_entry!(tracing::Level::TRACE, entry),
                "debug" => tracing_log_entry!(tracing::Level::DEBUG, entry),
                "info" => tracing_log_entry!(tracing::Level::INFO, entry),
                "warn" => tracing_log_entry!(tracing::Level::WARN, entry),
                "error" => tracing_log_entry!(tracing::Level::ERROR, entry),
                _ => tracing_log_entry!(tracing::Level::INFO, entry),
            }
        }
    }
}
