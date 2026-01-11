use async_trait::async_trait;
use bytes::Bytes;
use http::{Request, Response};

use super::{Error, Result};
use crate::proxy;

pub struct Client {
    client: reqwest::Client,
}

impl Client {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new(reqwest::Client::new())
    }
}

#[async_trait]
impl proxy::Client for Client {
    async fn send(&self, request: Request<Bytes>) -> Result<Response<Bytes>> {
        // Convert http::Request to reqwest::Request
        let (parts, body) = request.into_parts();

        let req = self
            .client
            .request(parts.method, parts.uri.to_string())
            .headers(parts.headers)
            .body(body);

        // Send request
        let resp = req.send().await.map_err(|e| Error::Client(Box::new(e)))?;

        // Build http::Response
        let status = resp.status();
        let headers = resp.headers().clone();
        let body = resp.bytes().await.map_err(|e| Error::Client(Box::new(e)))?;

        let mut response = Response::builder().status(status).body(body)?;

        *response.headers_mut() = headers;

        Ok(response)
    }
}
