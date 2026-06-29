//! The query host-service error (query scope). Maps onto [`ToolError`] at the MCP bridge: `Denied`
//! stays opaque (the capability/workspace deny reveals nothing); author feedback — a not-found query,
//! a bad target, a malformed PRQL, a param mismatch — surfaces as distinguishable `BadInput`.

use lb_mcp::ToolError;

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    /// Authorization failed (workspace isolation or a missing capability). Opaque by design — the
    /// headline no-widening deny (`query.run` without the underlying target cap) collapses here too.
    #[error("denied")]
    Denied,
    /// The named query is not saved in this workspace (un-spoofable: a cross-tenant id resolves to
    /// nothing here).
    #[error("no such query")]
    NotFound,
    /// Bad input to a verb (a missing field, an unknown lang/target, a param mismatch).
    #[error("bad input: {0}")]
    BadInput(String),
    /// The PRQL failed to compile (author feedback, surfaced verbatim from `lb-prql`).
    #[error("compile error: {0}")]
    Compile(String),
    #[error(transparent)]
    Store(#[from] lb_store::StoreError),
}

impl From<QueryError> for ToolError {
    fn from(e: QueryError) -> Self {
        match e {
            QueryError::Denied => ToolError::Denied,
            QueryError::NotFound => ToolError::BadInput("no such query".into()),
            QueryError::BadInput(m) => ToolError::BadInput(m),
            QueryError::Compile(m) => ToolError::BadInput(format!("compile error: {m}")),
            QueryError::Store(s) => ToolError::Extension(s.to_string()),
        }
    }
}
