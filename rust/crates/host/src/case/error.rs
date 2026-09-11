//! The case service error — `Denied` is opaque (§3.5), like every other service's denial. Mirrors
//! `InsightSvcError`'s shape so the host bridge maps them uniformly.

use thiserror::Error;

/// The case service error. Mapped to the MCP `ToolError` by the bridge — denials opaque, bad-input
/// surfaced (so a caller can supply the missing `resolution` and retry).
#[derive(Debug, Error)]
pub enum CaseSvcError {
    /// Authorization failed (workspace isolation or missing capability). Opaque by design.
    #[error("denied")]
    Denied,
    /// The caller-supplied input was malformed or violated an invariant.
    #[error("bad input: {0}")]
    BadInput(String),
    /// The durable store rejected the operation.
    #[error("store error: {0}")]
    Store(String),
}

impl From<lb_cases::CasesError> for CaseSvcError {
    fn from(e: lb_cases::CasesError) -> Self {
        match e {
            lb_cases::CasesError::BadInput(m) => CaseSvcError::BadInput(m),
            lb_cases::CasesError::Store(s) => CaseSvcError::Store(s.to_string()),
        }
    }
}

impl From<lb_store::StoreError> for CaseSvcError {
    fn from(e: lb_store::StoreError) -> Self {
        CaseSvcError::Store(e.to_string())
    }
}

impl From<lb_insights::InsightsError> for CaseSvcError {
    fn from(e: lb_insights::InsightsError) -> Self {
        match e {
            lb_insights::InsightsError::BadInput(m) => CaseSvcError::BadInput(m),
            lb_insights::InsightsError::Store(s) => CaseSvcError::Store(s.to_string()),
        }
    }
}
