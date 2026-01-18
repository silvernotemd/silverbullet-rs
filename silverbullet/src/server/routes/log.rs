use axum::extract::{FromRef, State};
use axum_client_ip::ClientIp;
use http::StatusCode;

use crate::client::{self, LogEntry};

pub trait Provider {
    type Output: client::Logger + Send + Sync;

    fn provide(&self) -> Self::Output;
}

pub struct Logger<L>(pub L);

impl<S> FromRef<S> for Logger<S::Output>
where
    S: Provider + Send + Sync,
{
    fn from_ref(state: &S) -> Self {
        Logger(state.provide())
    }
}

pub async fn log<L>(
    State(Logger(logger)): State<Logger<L>>,
    ClientIp(ip): ClientIp,
    axum::Json(entries): axum::Json<Vec<LogEntry>>,
) -> StatusCode
where
    L: client::Logger,
{
    logger.log(ip.to_string(), entries);

    StatusCode::OK
}
