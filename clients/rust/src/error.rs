//! Errors — the gateway's deny path is opaque (`401` / `403` / `404` with no
//! body that distinguishes "missing cap" from "missing record"), so this type
//! surfaces the status + body verbatim and lets the caller decide.

use thiserror::Error;

/// A structured failure from the gateway (a non-2xx response), carrying the
/// status code and the raw body so a caller can branch on "denied" vs "bad
/// input" without us guessing.
#[derive(Debug, Error)]
pub struct ApiError {
    pub status: u16,
    pub body: String,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "gateway returned {}: {}", self.status, self.body)
    }
}

impl ApiError {
    /// `true` for the opaque capability-deny / workspace-wall statuses. The
    /// common "the call was rejected" branch a caller wants to single out.
    pub fn is_denied(&self) -> bool {
        matches!(self.status, 401 | 403 | 404)
    }
}

/// Anything that can go wrong calling the gateway.
#[derive(Debug, Error)]
pub enum LbError {
    #[error("{0}")]
    Api(#[from] ApiError),
    #[error("transport error: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("invalid UTF-8 in response body: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
}
