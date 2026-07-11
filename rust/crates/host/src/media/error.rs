//! Media errors (media scope).

use lb_mcp::ToolError;

#[derive(Debug)]
pub enum MediaError {
    Denied,
    NotFound,
    TooLarge,
    BadChecksum,
    MissingChunks,
    NotReady,
    BadInput(String),
    Store(String),
}

impl std::fmt::Display for MediaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Denied => write!(f, "denied"),
            Self::NotFound => write!(f, "media not found"),
            Self::TooLarge => write!(f, "media exceeds size limit"),
            Self::BadChecksum => write!(f, "checksum mismatch"),
            Self::MissingChunks => write!(f, "missing chunks"),
            Self::NotReady => write!(f, "media not ready"),
            Self::BadInput(s) => write!(f, "bad input: {s}"),
            Self::Store(s) => write!(f, "store error: {s}"),
        }
    }
}

impl From<lb_store::StoreError> for MediaError {
    fn from(e: lb_store::StoreError) -> Self {
        Self::Store(e.to_string())
    }
}

impl From<MediaError> for ToolError {
    fn from(e: MediaError) -> Self {
        match e {
            MediaError::Denied => ToolError::Denied,
            MediaError::NotFound | MediaError::NotReady => ToolError::NotFound,
            MediaError::TooLarge
            | MediaError::BadChecksum
            | MediaError::MissingChunks
            | MediaError::BadInput(_) => ToolError::BadInput(e.to_string()),
            MediaError::Store(s) => ToolError::Extension(s),
        }
    }
}
