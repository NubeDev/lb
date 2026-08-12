//! The async HTTP client core: header/auth setup + the shared `get_json`/`patch_json` helpers. Ported
//! from the blocking `rust-ros` client to `reqwest::Client` (async) — the poller runs many concurrent
//! reads, so a blocking client on the async runtime would stall the reactor (ros-scope risk "blocking
//! client in an async task"). The `External {token}` auth header is the ROS appliance's scheme; the
//! token itself is mediated by `lb-secrets` above this layer and never logged here.
//!
//! **One process-wide `reqwest::Client`, not one per connection.** `resolve_api` rebuilds a `Client`
//! on EVERY tool call (it re-reads the shadow + token fresh each time — correct, cheap, and lets a
//! rotated token take effect immediately). Baking the token into `reqwest::Client`'s DEFAULT headers
//! (the old shape) forced a brand-new client — fresh connection pool, fresh TCP+TLS handshake to the
//! box — on every single call; a Location→Group→Host→Network→Device→Point chain paid that six times
//! over, the dominant cost in the "why is this slow" report. `reqwest::Client` is `Arc`-backed and
//! `Clone` is a cheap handle copy sharing the SAME pool, so the fix is to build ONE client for the
//! process (`shared_http`, no default auth header — a token is per-connection, not per-process) and
//! send `Authorization` as a PER-REQUEST header instead. Every `Client` now clones the same pooled
//! handle, so requests to the SAME box (any two calls in one chain, or across different connections'
//! calls interleaved by the poller) reuse a warm keep-alive connection.

use std::sync::OnceLock;

use reqwest::header::{HeaderValue, AUTHORIZATION};
use reqwest::{Client as HttpClient, Response};
use serde::de::DeserializeOwned;

use super::error::RosClientError;

#[derive(Debug, Clone)]
pub struct Config {
    pub base_url: String,
    pub token: String,
}

#[derive(Debug, Clone)]
pub struct Client {
    http: HttpClient,
    base_url: String,
    token: String,
}

impl Client {
    pub fn new(config: Config) -> Result<Self, RosClientError> {
        Ok(Self {
            http: shared_http().clone(),
            base_url: config.base_url.trim_end_matches('/').to_string(),
            token: config.token,
        })
    }

    pub fn http_client(&self) -> &HttpClient {
        &self.http
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub(crate) fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn set_token(&mut self, token: impl Into<String>) -> Result<(), RosClientError> {
        self.token = token.into();
        Ok(())
    }

    fn auth_header(&self) -> Result<HeaderValue, RosClientError> {
        HeaderValue::from_str(&format!("External {}", self.token))
            .map_err(|e| RosClientError::InvalidInput(format!("invalid token/header: {e}")))
    }

    pub(crate) async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T, RosClientError> {
        let response = self
            .http
            .get(self.endpoint_url(path))
            .header(AUTHORIZATION, self.auth_header()?)
            .query(query)
            .send()
            .await?;
        Self::decode_json_response(response).await
    }

    pub(crate) async fn patch_json<B: serde::Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, RosClientError> {
        let response = self
            .http
            .patch(self.endpoint_url(path))
            .header(AUTHORIZATION, self.auth_header()?)
            .json(body)
            .send()
            .await?;
        Self::decode_json_response(response).await
    }

    pub(crate) fn endpoint_url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    pub(crate) async fn decode_json_response<T: DeserializeOwned>(
        response: Response,
    ) -> Result<T, RosClientError> {
        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<failed to read body>".to_string());
            Err(RosClientError::Api { status, body })
        }
    }
}

/// The one `reqwest::Client` for the whole sidecar process — built lazily on first use, then cloned
/// (an `Arc`-backed handle, sharing the same connection pool) by every `Client::new`. No default auth
/// header: the token is per-connection and goes on each request instead (see module doc). JSON
/// content-type is fixed per-request via `reqwest`'s `.json(body)` builder already, so the only default
/// header worth pre-baking here would be `Content-Type` on GETs — not needed, `.json(body)` sets it
/// per-call and `get_json` sends no body.
fn shared_http() -> &'static HttpClient {
    static HTTP: OnceLock<HttpClient> = OnceLock::new();
    HTTP.get_or_init(|| {
        HttpClient::builder()
            .build()
            .expect("reqwest client builder failed")
    })
}
