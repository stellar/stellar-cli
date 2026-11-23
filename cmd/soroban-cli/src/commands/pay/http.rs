//! Minimal HTTP client for the x402 request/replay loop.

use std::time::Duration;

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

/// Most redirects to follow, matching reqwest's default policy.
const MAX_REDIRECTS: usize = 10;
/// How long to wait for the server to accept the connection.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// How long to wait for more data from the server before giving up. Large
/// resources still download as long as data keeps arriving.
const READ_TIMEOUT: Duration = Duration::from_secs(30);

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
    let mut builder = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT);
    if payment_signature.is_some() {
        builder = builder.redirect(same_origin_redirects());
    }
    let client = builder.build().map_err(Error::Client)?;
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

/// A redirect policy that refuses to leave the original origin, so the payment
/// proof is never sent to a host other than the one that asked for it.
fn same_origin_redirects() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        let same_origin = attempt
            .previous()
            .first()
            .is_some_and(|first| first.origin() == attempt.url().origin());
        if !same_origin {
            let message = format!("refusing to send the payment to {}", attempt.url());
            attempt.error(message)
        } else if attempt.previous().len() > MAX_REDIRECTS {
            attempt.error("too many redirects")
        } else {
            attempt.follow()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn payment_is_not_sent_to_another_host_on_redirect() {
        let mut origin = mockito::Server::new_async().await;
        let mut other = mockito::Server::new_async().await;
        origin
            .mock("GET", "/pay")
            .with_status(302)
            .with_header("location", &format!("{}/steal", other.url()))
            .create_async()
            .await;
        let stolen = other
            .mock("GET", "/steal")
            .with_status(200)
            .expect(0)
            .create_async()
            .await;

        let result = get(&format!("{}/pay", origin.url()), Some("proof")).await;

        stolen.assert_async().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn payment_follows_a_same_host_redirect() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/pay")
            .with_status(302)
            .with_header("location", "/paid")
            .create_async()
            .await;
        let paid = server
            .mock("GET", "/paid")
            .match_header(PAYMENT_SIGNATURE_HEADER, "proof")
            .with_status(200)
            .with_body("ok")
            .create_async()
            .await;

        let response = get(&format!("{}/pay", server.url()), Some("proof"))
            .await
            .unwrap();

        paid.assert_async().await;
        assert_eq!(response.body, b"ok");
    }
}
