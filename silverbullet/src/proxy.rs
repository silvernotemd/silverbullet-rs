use async_trait::async_trait;
use bytes::Bytes;
use http::{HeaderMap, HeaderName, HeaderValue, Request, Response, StatusCode};
use thiserror::Error;

#[cfg(feature = "reqwest")]
pub mod reqwest;

// #[cfg(feature = "proxy-cloudflare")]
// pub mod cloudflare;

// Platform-specific error boxing
#[cfg(not(target_arch = "wasm32"))]
type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[cfg(target_arch = "wasm32")]
type BoxError = Box<dyn std::error::Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("HTTP client error: {0}")]
    Client(#[source] BoxError),

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Proxy not supported: {0}")]
    NotSupported(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] http::Error),

    #[error(transparent)]
    Other(#[from] BoxError),
}

pub type Result<T> = std::result::Result<T, Error>;

/// HTTP client trait for making requests
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait Client: Send + Sync {
    async fn send(&self, request: Request<Bytes>) -> Result<Response<Bytes>>;
}

/// Proxy handles proxying HTTP requests through a client
pub struct Proxy<C> {
    client: C,
}

impl<C> Proxy<C>
where
    C: Client,
{
    pub fn new(client: C) -> Self {
        Self { client }
    }

    /// Proxy an HTTP request
    ///
    /// Extracts the target URL from the request path (everything after /.proxy/),
    /// filters headers, adjusts the scheme, and forwards the request.
    pub async fn proxy(&self, request: Request<Bytes>) -> Result<Response<Bytes>> {
        let (parts, body) = request.into_parts();

        // Extract target URL from path (everything after /.proxy/)
        let path = parts.uri.path();
        let mut target_url = path.strip_prefix("/.proxy/").unwrap_or(path).to_string();

        // Append query parameters if present
        if let Some(query) = parts.uri.query() {
            target_url.push('?');
            target_url.push_str(query);
        }

        // Adjust scheme (http for localhost/IPs, https otherwise)
        target_url = adjust_scheme(&target_url);

        // Filter headers (only forward x-proxy-header-* with prefix stripped)
        let filtered_headers = filter_proxy_headers(&parts.headers);

        // Build proxied request
        let mut proxied_request = Request::builder()
            .method(parts.method)
            .uri(target_url)
            .body(body)?;

        *proxied_request.headers_mut() = filtered_headers;

        // Send the request
        let upstream_response = self.client.send(proxied_request).await?;

        // Process response headers
        let (upstream_parts, upstream_body) = upstream_response.into_parts();

        let mut response_headers = HeaderMap::new();

        // Add status code as header
        let status_value = HeaderValue::from_str(&upstream_parts.status.as_u16().to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("500"));
        response_headers.insert(HeaderName::from_static("x-proxy-status-code"), status_value);

        // Extract content-type before iterating
        let content_type = upstream_parts
            .headers
            .get(http::header::CONTENT_TYPE)
            .cloned();

        // Add all response headers with x-proxy-header- prefix
        for (name, value) in upstream_parts.headers.iter() {
            let prefixed = format!("x-proxy-header-{}", name.as_str());
            if let Ok(header_name) = prefixed.parse::<HeaderName>() {
                response_headers.insert(header_name, value.clone());
            }
        }

        // Set content-type explicitly (without prefix)
        if let Some(ct) = content_type {
            response_headers.insert(http::header::CONTENT_TYPE, ct);
        }

        // Build response - always return 200 with actual status in header
        Response::builder()
            .status(StatusCode::OK)
            .body(upstream_body)
            .map(|mut response| {
                *response.headers_mut() = response_headers;
                response
            })
            .map_err(Error::from)
    }
}

fn filter_proxy_headers(headers: &HeaderMap) -> HeaderMap {
    use std::str::FromStr;

    headers
        .iter()
        .filter_map(|(name, value)| {
            let name_str = name.as_str();
            let name_lower = name_str.to_ascii_lowercase();

            if name_lower.starts_with("x-proxy-header-") {
                let stripped = &name_str["x-proxy-header-".len()..];
                Some((HeaderName::from_str(stripped).ok()?, value.clone()))
            } else {
                None
            }
        })
        .collect()
}

fn adjust_scheme(url: &str) -> String {
    // Check for IPv6 localhost first (before splitting on ':')
    if url.starts_with("::1") || url.starts_with("[::1]") {
        return format!("http://{}", url);
    }

    // Extract host from URL (before first / or :)
    let host = url
        .split('/')
        .next()
        .unwrap_or(url)
        .split(':')
        .next()
        .unwrap_or(url);

    // Check if host is local/private
    let use_http = host == "localhost"
        || host == "127.0.0.1"
        || host == "host.docker.internal"
        || is_private_ip(host);

    if use_http {
        format!("http://{}", url)
    } else {
        format!("https://{}", url)
    }
}

fn is_private_ip(host: &str) -> bool {
    // Check for private IP ranges
    host.starts_with("192.168.")
        || host.starts_with("10.")
        || host.starts_with("172.16.")
        || host.starts_with("172.17.")
        || host.starts_with("172.18.")
        || host.starts_with("172.19.")
        || host.starts_with("172.20.")
        || host.starts_with("172.21.")
        || host.starts_with("172.22.")
        || host.starts_with("172.23.")
        || host.starts_with("172.24.")
        || host.starts_with("172.25.")
        || host.starts_with("172.26.")
        || host.starts_with("172.27.")
        || host.starts_with("172.28.")
        || host.starts_with("172.29.")
        || host.starts_with("172.30.")
        || host.starts_with("172.31.")
}

/// No-op proxy implementation that returns NotSupported error
#[derive(Debug, Default)]
pub struct NoProxy;

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl Client for NoProxy {
    async fn send(&self, _request: Request<Bytes>) -> Result<Response<Bytes>> {
        Err(Error::NotSupported(
            "Proxy feature not available".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_proxy_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("x-proxy-header-authorization", "Bearer token".parse().unwrap());
        headers.insert("x-proxy-header-content-type", "application/json".parse().unwrap());
        headers.insert("regular-header", "should-be-filtered".parse().unwrap());
        headers.insert("X-Proxy-Header-Custom", "custom-value".parse().unwrap());

        let filtered = filter_proxy_headers(&headers);

        assert_eq!(filtered.len(), 3);
        assert_eq!(filtered.get("authorization").unwrap(), "Bearer token");
        assert_eq!(filtered.get("content-type").unwrap(), "application/json");
        assert_eq!(filtered.get("custom").unwrap(), "custom-value");
        assert!(filtered.get("regular-header").is_none());
    }

    #[test]
    fn test_adjust_scheme_localhost() {
        assert_eq!(adjust_scheme("localhost:3000/path"), "http://localhost:3000/path");
        assert_eq!(adjust_scheme("127.0.0.1:8080"), "http://127.0.0.1:8080");
        assert_eq!(adjust_scheme("::1/path"), "http://::1/path");
        assert_eq!(adjust_scheme("host.docker.internal"), "http://host.docker.internal");
    }

    #[test]
    fn test_adjust_scheme_private_ips() {
        assert_eq!(adjust_scheme("192.168.1.1"), "http://192.168.1.1");
        assert_eq!(adjust_scheme("10.0.0.1"), "http://10.0.0.1");
        assert_eq!(adjust_scheme("172.16.0.1"), "http://172.16.0.1");
        assert_eq!(adjust_scheme("172.31.255.255"), "http://172.31.255.255");
    }

    #[test]
    fn test_adjust_scheme_public() {
        assert_eq!(adjust_scheme("example.com"), "https://example.com");
        assert_eq!(adjust_scheme("api.github.com/repos"), "https://api.github.com/repos");
        assert_eq!(adjust_scheme("1.1.1.1"), "https://1.1.1.1");
    }

    #[test]
    fn test_is_private_ip() {
        // Private ranges
        assert!(is_private_ip("192.168.0.1"));
        assert!(is_private_ip("192.168.255.255"));
        assert!(is_private_ip("10.0.0.0"));
        assert!(is_private_ip("10.255.255.255"));
        assert!(is_private_ip("172.16.0.0"));
        assert!(is_private_ip("172.31.255.255"));

        // Public IPs
        assert!(!is_private_ip("8.8.8.8"));
        assert!(!is_private_ip("1.1.1.1"));
        assert!(!is_private_ip("172.15.0.0"));
        assert!(!is_private_ip("172.32.0.0"));
        assert!(!is_private_ip("192.167.0.0"));
    }

    #[tokio::test]
    async fn test_no_proxy_returns_not_supported() {
        let client = NoProxy;
        let request = Request::builder()
            .uri("http://example.com")
            .body(Bytes::new())
            .unwrap();

        let result = client.send(request).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            Error::NotSupported(msg) => {
                assert_eq!(msg, "Proxy feature not available");
            }
            _ => panic!("Expected NotSupported error"),
        }
    }

    // Mock client for testing Proxy
    struct MockClient {
        response: Response<Bytes>,
    }

    #[async_trait]
    impl Client for MockClient {
        async fn send(&self, _request: Request<Bytes>) -> Result<Response<Bytes>> {
            Ok(self.response.clone())
        }
    }

    #[tokio::test]
    async fn test_proxy_url_extraction() {
        let mock_response = Response::builder()
            .status(StatusCode::OK)
            .body(Bytes::from("response body"))
            .unwrap();

        let client = MockClient {
            response: mock_response,
        };
        let proxy = Proxy::new(client);

        let request = Request::builder()
            .uri("/.proxy/example.com/api/endpoint?foo=bar")
            .body(Bytes::new())
            .unwrap();

        let result = proxy.proxy(request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_proxy_header_filtering() {
        let mock_response = Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .header("x-custom", "value")
            .body(Bytes::from("{}"))
            .unwrap();

        let client = MockClient {
            response: mock_response,
        };
        let proxy = Proxy::new(client);

        let request = Request::builder()
            .uri("/.proxy/example.com")
            .header("x-proxy-header-authorization", "Bearer token")
            .header("regular-header", "filtered")
            .body(Bytes::new())
            .unwrap();

        let response = proxy.proxy(request).await.unwrap();

        // Check response has x-proxy-status-code
        assert_eq!(
            response.headers().get("x-proxy-status-code").unwrap(),
            "200"
        );

        // Check response has prefixed headers
        assert_eq!(
            response.headers().get("x-proxy-header-x-custom").unwrap(),
            "value"
        );

        // Check content-type is unprefixed
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "application/json"
        );

        // Response should always be 200
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_proxy_wraps_upstream_status() {
        let mock_response = Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Bytes::from("Not found"))
            .unwrap();

        let client = MockClient {
            response: mock_response,
        };
        let proxy = Proxy::new(client);

        let request = Request::builder()
            .uri("/.proxy/example.com/missing")
            .body(Bytes::new())
            .unwrap();

        let response = proxy.proxy(request).await.unwrap();

        // Proxy always returns 200
        assert_eq!(response.status(), StatusCode::OK);

        // Actual status is in header
        assert_eq!(
            response.headers().get("x-proxy-status-code").unwrap(),
            "404"
        );
    }
}
