//! The `Client` — base URL + bearer credential + the one HTTP plumbing function
//! the other verbs share. The bearer is opaque to this library; see the crate
//! doc for why.

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::{Client as Http, Method, Response};
use serde::{Deserialize, Serialize};

use crate::error::{ApiError, LbError};

/// A configured gateway client. Clone is cheap (the `reqwest::Client` and the
/// bearer string are both cheap to clone). The bearer is sent as
/// `Authorization: Bearer <bearer>` on every authenticated call.
#[derive(Clone)]
pub struct Client {
    base_url: String,
    bearer: String,
    http: Http,
}

impl Client {
    /// Construct from a base URL (e.g. `http://127.0.0.1:8080`) and a bearer
    /// credential — either an API key `lbk_{ws}.{id}.{secret}` or a JWT. Read
    /// the key from an env var in real code; do not hard-code it.
    pub fn new(base_url: impl Into<String>, bearer: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            bearer: bearer.into(),
            http: Http::new(),
        }
    }

    /// The base URL this client points at.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// A clone of the inner `reqwest::Client`. Exposed so the webhook helper
    /// can POST to `/hooks/{ws}/{id}` WITHOUT the bearer (the inbound webhook
    /// route is the one gateway route that takes no session token).
    pub(crate) fn http_handle(&self) -> Http {
        self.http.clone()
    }

    /// `POST /login {user, workspace}` — the dev-login path. Use for local-dev
    /// / admin scripts; for a long-lived producer, mint an API key once via the
    /// admin console (or `POST /admin/apikeys`) and use [`Client::new`] with it.
    /// Returns a NEW `Client` carrying the issued session token.
    pub async fn login(
        &self,
        user: &str,
        workspace: &str,
    ) -> Result<(Client, LoginReply), LbError> {
        let body = serde_json::json!({ "user": user, "workspace": workspace });
        let resp = self.request(Method::POST, "/login").json(&body).send().await?;
        let reply: LoginReply = decode(resp).await?;
        Ok((self.with_bearer(&reply.token), reply))
    }

    /// Replace the bearer (used by `login`; also useful for rotation).
    pub fn with_bearer(&self, bearer: &str) -> Client {
        Client {
            base_url: self.base_url.clone(),
            bearer: bearer.to_string(),
            http: self.http.clone(),
        }
    }

    /// Begin a builder for `path` (e.g. `"/ingest"`) under `method`. Carries
    /// the bearer. Use the typed verbs in `ingest` / `mcp` / `webhook` rather
    /// than calling this directly — it is `pub(crate)` for the few cases the
    /// caller needs a route the library doesn't wrap.
    pub(crate) fn request(
        &self,
        method: Method,
        path: &str,
    ) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        let mut headers = HeaderMap::new();
        // SAFETY: the bearer comes from the caller; a non-ASCII byte sequence
        // would have failed key generation long before reaching us. We trim so
        // a stray newline from an env var is not forwarded.
        let value = format!("Bearer {}", self.bearer.trim());
        if let Ok(hv) = HeaderValue::from_str(&value) {
            headers.insert(AUTHORIZATION, hv);
        }
        self.http
            .request(method, url)
            .header("accept", "application/json")
            .headers(headers)
    }
}

/// The `POST /login` reply (see `routes/login.rs::LoginReply`).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoginReply {
    pub token: String,
    pub principal: String,
    pub workspace: String,
    #[serde(default)]
    pub caps: Vec<String>,
}

/// Decode a response: 2xx → JSON, else build an [`ApiError`] with the raw body.
pub(crate) async fn decode<T: for<'de> Deserialize<'de>>(resp: Response) -> Result<T, LbError> {
    let status = resp.status();
    let bytes = resp.bytes().await.map_err(LbError::Transport)?;
    if !status.is_success() {
        let body = String::from_utf8_lossy(&bytes).into_owned();
        return Err(LbError::Api(ApiError {
            status: status.as_u16(),
            body,
        }));
    }
    Ok(serde_json::from_slice(&bytes).map_err(|e| {
        LbError::Api(ApiError {
            status: status.as_u16(),
            body: format!("invalid JSON: {e}"),
        })
    })?)
}
