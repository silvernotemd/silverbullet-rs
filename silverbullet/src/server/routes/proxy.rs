use axum::extract::{FromRef, Request, State};
use axum::response::{IntoResponse, Response};
use http_body_util::BodyExt;

use crate::proxy::{self, Client};

pub trait Provider {
    type Output: Client + Send + Sync;

    fn provide(&self) -> Self::Output;
}

pub struct Proxy<C>(pub proxy::Proxy<C>);

impl<S> FromRef<S> for Proxy<S::Output>
where
    S: Provider + Send + Sync,
{
    fn from_ref(state: &S) -> Self {
        Proxy(proxy::Proxy::new(state.provide()))
    }
}

#[cfg_attr(feature = "cloudflare", worker::send)]
pub async fn proxy<C>(
    State(Proxy(proxy)): State<Proxy<C>>,
    request: Request,
) -> Result<impl IntoResponse, Response>
where
    C: Client,
{
    // Collect body to Bytes
    let (parts, body) = request.into_parts();
    let body_bytes = body
        .collect()
        .await
        .map_err(|_| http::StatusCode::BAD_REQUEST.into_response())?
        .to_bytes();
    let request_with_bytes = http::Request::from_parts(parts, body_bytes);

    // Send through proxy
    let response = proxy.proxy(request_with_bytes).await.map_err(|e| {
        #[cfg(feature = "tracing")]
        tracing::error!("Proxy request failed: {}", e);

        // Check if it's a NotSupported error
        match e {
            proxy::Error::NotSupported(_) => http::StatusCode::NOT_IMPLEMENTED.into_response(),
            _ => http::StatusCode::BAD_GATEWAY.into_response(),
        }
    })?;

    // Convert Response<Bytes> to Response<Body> for axum
    let (parts, body_bytes) = response.into_parts();
    Ok(http::Response::from_parts(
        parts,
        axum::body::Body::from(body_bytes),
    ))
}
