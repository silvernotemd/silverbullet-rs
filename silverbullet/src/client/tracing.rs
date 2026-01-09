use crate::client::{LogEntry, Logger};

pub struct TracingLogger;

impl Default for TracingLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl TracingLogger {
    pub fn new() -> Self {
        TracingLogger
    }
}

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
