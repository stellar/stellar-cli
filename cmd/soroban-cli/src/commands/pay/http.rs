//! Minimal HTTP client for the x402 request/replay loop.

use reqwest::header::{HeaderMap, CONTENT_TYPE};

use super::challenge::PAYMENT_SIGNATURE_HEADER;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("could not reach `{url}`: {source}")]
    Request {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("failed to build the HTTP client: {0}")]
    Client(reqwest::Error),
}

impl Error {
    /// Machine-readable discriminator for the JSON error envelope's `type` field.
    #[must_use]
    pub fn error_type(&self) -> &'static str {
        match self {
            Error::Request { .. } => "request_failed",
            Error::Client(_) => "http_client",
        }
    }
}

/// A fetched HTTP response, with the body read into memory.
pub struct Response {
    pub status: u16,
    headers: HeaderMap,
    pub body: Vec<u8>,
}

impl Response {
    /// Read a response header by (case-insensitive) name, if present and valid
    /// UTF-8.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<String> {
        self.headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string)
    }

    /// The response's `Content-Type`, if any.
    #[must_use]
    pub fn content_type(&self) -> Option<String> {
        self.headers
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string)
    }

    /// The response body as a lossy UTF-8 string.
    #[must_use]
    pub fn body_string(&self) -> String {
        String::from_utf8_lossy(&self.body).to_string()
    }
}

/// `GET url`, optionally attaching the x402 `PAYMENT-SIGNATURE` proof header.
pub async fn get(url: &str, payment_signature: Option<&str>) -> Result<Response, Error> {
    let client = reqwest::Client::builder().build().map_err(Error::Client)?;
    let mut request = client.get(url);
    if let Some(signature) = payment_signature {
        request = request.header(PAYMENT_SIGNATURE_HEADER, signature);
    }
    let response = request.send().await.map_err(|source| Error::Request {
        url: url.to_string(),
        source,
    })?;
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let body = response
        .bytes()
        .await
        .map_err(|source| Error::Request {
            url: url.to_string(),
            source,
        })?
        .to_vec();
    Ok(Response {
        status,
        headers,
        body,
    })
}
