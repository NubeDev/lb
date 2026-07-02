//! The ROS REST client error type. Unchanged from the vendored `rust-ros` (async port touches only the
//! call sites, not the error shape): an HTTP/transport failure, a non-2xx API response (status + body
//! for a log line — never the token), or invalid local input.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RosClientError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("api error: status {status}, body: {body}")]
    Api { status: u16, body: String },

    #[error("invalid input: {0}")]
    InvalidInput(String),
}
