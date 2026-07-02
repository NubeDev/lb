//! The async HTTP client core: header/auth setup + the shared `get_json`/`patch_json` helpers. Ported
//! from the blocking `rust-ros` client to `reqwest::Client` (async) — the poller runs many concurrent
//! reads, so a blocking client on the async runtime would stall the reactor (ros-scope risk "blocking
//! client in an async task"). The `External {token}` auth header is the ROS appliance's scheme; the
//! token itself is mediated by `lb-secrets` above this layer and never logged here.

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
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
        let http = build_http(&config.token)?;
        Ok(Self {
            http,
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

    pub fn set_token(&mut self, token: impl Into<String>) -> Result<(), RosClientError> {
        let token = token.into();
        self.http = build_http(&token)?;
        self.token = token;
        Ok(())
    }

    pub(crate) async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T, RosClientError> {
        let response = self
            .http
            .get(self.endpoint_url(path))
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

/// Build a `reqwest::Client` carrying the ROS `External {token}` auth + JSON content-type headers.
fn build_http(token: &str) -> Result<HttpClient, RosClientError> {
    let mut headers = HeaderMap::new();
    let auth_value = format!("External {token}");
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&auth_value)
            .map_err(|e| RosClientError::InvalidInput(format!("invalid token/header: {e}")))?,
    );
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    HttpClient::builder()
        .default_headers(headers)
        .build()
        .map_err(RosClientError::from)
}
