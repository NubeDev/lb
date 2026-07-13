//! Notify errors (push-target scope).

use lb_mcp::ToolError;

#[derive(Debug)]
pub enum NotifyError {
    Denied,
    NotFound,
    BadInput(String),
    Store(String),
}

impl std::fmt::Display for NotifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Denied => write!(f, "denied"),
            Self::NotFound => write!(f, "not found"),
            Self::BadInput(s) => write!(f, "bad input: {s}"),
            Self::Store(s) => write!(f, "store error: {s}"),
        }
    }
}

impl From<lb_store::StoreError> for NotifyError {
    fn from(e: lb_store::StoreError) -> Self {
        Self::Store(e.to_string())
    }
}

impl From<NotifyError> for ToolError {
    fn from(e: NotifyError) -> Self {
        match e {
            NotifyError::Denied => ToolError::Denied,
            NotifyError::NotFound => ToolError::NotFound,
            NotifyError::BadInput(s) => ToolError::BadInput(s),
            NotifyError::Store(s) => ToolError::Extension(s),
        }
    }
}
