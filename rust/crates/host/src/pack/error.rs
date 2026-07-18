//! The `pack.*` family error. Every downstream seam's error collapses into one of these — the
//! bridge maps them to `ToolError` exactly as the other families do.

use lb_mcp::ToolError;

#[derive(Debug)]
pub enum PackError {
    /// The caller lacks `mcp:pack.<verb>:call`, or the workspace gate refused. Opaque by design.
    Denied,
    /// No receipt for that pack in this workspace.
    NotFound,
    /// The bundle is malformed, oversize, or the manifest is invalid — the message is the author's.
    BadInput(String),
    /// The pack is self-inconsistent (a lint ERROR gated the apply).
    Invalid(Vec<String>),
    /// A re-apply the refusal matrix refused, with its reason.
    Refused(String),
    /// Something below the seam failed.
    Internal(String),
}

impl From<PackError> for ToolError {
    fn from(e: PackError) -> ToolError {
        match e {
            PackError::Denied => ToolError::Denied,
            PackError::NotFound => ToolError::NotFound,
            PackError::BadInput(m) => ToolError::BadInput(m),
            PackError::Invalid(errs) => {
                ToolError::BadInput(format!("pack is invalid: {}", errs.join("; ")))
            }
            PackError::Refused(why) => ToolError::BadInput(format!("re-apply refused: {why}")),
            PackError::Internal(m) => ToolError::Extension(m),
        }
    }
}

impl std::fmt::Display for PackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackError::Denied => write!(f, "denied"),
            PackError::NotFound => write!(f, "not found"),
            PackError::BadInput(m) => write!(f, "bad input: {m}"),
            PackError::Invalid(e) => write!(f, "invalid pack: {}", e.join("; ")),
            PackError::Refused(w) => write!(f, "refused: {w}"),
            PackError::Internal(m) => write!(f, "internal: {m}"),
        }
    }
}

impl std::error::Error for PackError {}
