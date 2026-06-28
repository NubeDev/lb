//! The federation host-service error (datasources scope). Maps onto [`ToolError`] at the MCP bridge:
//! `Denied` stays opaque (the capability/workspace deny reveals nothing), while a not-found source, a
//! refused endpoint, or a sidecar fault surface as distinguishable client errors.

use lb_mcp::ToolError;

#[derive(Debug, thiserror::Error)]
pub enum FederationError {
    /// Authorization failed (workspace isolation or missing capability). Opaque by design.
    #[error("denied")]
    Denied,
    /// The named datasource is not registered in this workspace (un-spoofable: a caller cannot name
    /// another tenant's source — it simply resolves to nothing here).
    #[error("no such datasource")]
    NotFound,
    /// The endpoint the source connects to is not in the admin-approved `net:*` grant — refused,
    /// opaque, even with the binary present (the headline reference-extension deny).
    #[error("endpoint not permitted")]
    EndpointRefused,
    /// The supplied SQL is not a single SELECT (read-first v1).
    #[error("rejected sql: {0}")]
    BadSql(String),
    /// Bad input to a verb.
    #[error("bad input: {0}")]
    BadInput(String),
    /// The sidecar ran but returned an error or is not running.
    #[error("federation sidecar: {0}")]
    Sidecar(String),
    #[error(transparent)]
    Store(#[from] lb_store::StoreError),
}

impl From<FederationError> for ToolError {
    fn from(e: FederationError) -> Self {
        match e {
            FederationError::Denied => ToolError::Denied,
            FederationError::NotFound => ToolError::BadInput("no such datasource".into()),
            FederationError::EndpointRefused => ToolError::Denied,
            FederationError::BadSql(m) => ToolError::BadInput(format!("rejected sql: {m}")),
            FederationError::BadInput(m) => ToolError::BadInput(m),
            FederationError::Sidecar(m) => ToolError::Extension(m),
            FederationError::Store(s) => ToolError::Extension(s.to_string()),
        }
    }
}
