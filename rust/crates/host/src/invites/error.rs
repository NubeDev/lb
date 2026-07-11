//! Invite errors (invites scope).

use lb_mcp::ToolError;

#[derive(Debug)]
pub enum InviteError {
    Denied,
    NotFound,
    Expired,
    AlreadyAccepted,
    Revoked,
    BadToken,
    BadInput(String),
    IdentityExists(String),
    Store(String),
}

impl std::fmt::Display for InviteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Denied => write!(f, "denied"),
            Self::NotFound => write!(f, "invite not found"),
            Self::Expired => write!(f, "invite expired"),
            Self::AlreadyAccepted => write!(f, "invite already accepted"),
            Self::Revoked => write!(f, "invite revoked"),
            Self::BadToken => write!(f, "bad invite token"),
            Self::BadInput(msg) => write!(f, "bad input: {msg}"),
            Self::IdentityExists(msg) => write!(f, "identity exists: {msg}"),
            Self::Store(s) => write!(f, "store error: {s}"),
        }
    }
}

impl From<lb_store::StoreError> for InviteError {
    fn from(e: lb_store::StoreError) -> Self {
        Self::Store(e.to_string())
    }
}

impl From<InviteError> for ToolError {
    fn from(e: InviteError) -> Self {
        match e {
            InviteError::Denied => ToolError::Denied,
            InviteError::BadToken
            | InviteError::Expired
            | InviteError::AlreadyAccepted
            | InviteError::Revoked
            | InviteError::NotFound => ToolError::BadInput(e.to_string()),
            InviteError::BadInput(msg) | InviteError::IdentityExists(msg) => {
                ToolError::BadInput(msg)
            }
            InviteError::Store(s) => ToolError::Extension(s),
        }
    }
}
