use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

pub struct Error(Box<dyn std::error::Error + Send + Sync>);

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        #[cfg(feature = "tracing")]
        tracing::error!(error = %self.0, error.source = ?self.0.source(), "Internal server error");

        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

impl<E> From<E> for Error
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn from(err: E) -> Self {
        Error(Box::new(err))
    }
}

impl From<Error> for Response {
    fn from(err: Error) -> Self {
        err.into_response()
    }
}
