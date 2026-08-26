//! The mail-source service error. `Denied` carries no detail — a caller without the grant learns
//! nothing about which sources exist (the same opacity every other host service keeps).

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MailSourceError {
    /// Authorization failed (workspace isolation or missing capability). Opaque by design.
    #[error("denied")]
    Denied,
    /// The arguments could not describe a mailbox.
    #[error("bad input: {0}")]
    BadInput(String),
    /// No source with that id in this workspace.
    #[error("not found")]
    NotFound,
    /// The mailbox could not be reached, or refused us. Carries the transport's already-redacted
    /// message and its retry classification — a caller running `mail.source.check` needs to know
    /// whether to fix the config or wait.
    #[error("mailbox unreachable: {message}")]
    Transport { message: String, permanent: bool },
    /// The durable store rejected the operation.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}

impl From<lb_mail::MailError> for MailSourceError {
    fn from(error: lb_mail::MailError) -> Self {
        MailSourceError::Transport {
            permanent: error.is_permanent(),
            // Already redacted by `lb-mail` on the way out; nothing here re-derives it.
            message: error.message().to_string(),
        }
    }
}
