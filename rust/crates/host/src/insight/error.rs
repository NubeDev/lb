//! The insights service error — `Denied` is opaque (§3.5), like every other service's denial.
//! Mirrors `InboxError`/`OutboxError`'s shape so the host bridge maps them uniformly.

use thiserror::Error;

/// The insights service error. Mapped to the MCP `ToolError` by the bridge — denials opaque,
/// bad-input surfaced (so a producer can slim an oversize occurrence + retry).
#[derive(Debug, Error)]
pub enum InsightSvcError {
    /// Authorization failed (workspace isolation or missing capability). Opaque by design.
    #[error("denied")]
    Denied,
    /// The caller-supplied input was malformed or violated a hard limit.
    #[error("bad input: {0}")]
    BadInput(String),
    /// The durable store rejected the operation.
    #[error("store error: {0}")]
    Store(String),
}

impl From<lb_insights::InsightsError> for InsightSvcError {
    fn from(e: lb_insights::InsightsError) -> Self {
        match e {
            lb_insights::InsightsError::BadInput(m) => InsightSvcError::BadInput(m),
            lb_insights::InsightsError::Store(s) => InsightSvcError::Store(s.to_string()),
        }
    }
}

impl From<lb_store::StoreError> for InsightSvcError {
    fn from(e: lb_store::StoreError) -> Self {
        InsightSvcError::Store(e.to_string())
    }
}
