//! The reminders service error. Same discipline as the sibling crates: a denial is opaque (no
//! existence signal), store errors carry through, and the one new outcome is a malformed cron
//! schedule ([`BadCron`](ReminderError::BadCron)) surfaced as a `BadInput` at the MCP boundary.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReminderError {
    /// A reminder capability gate refused. Opaque — the caller cannot tell "not allowed" from
    /// "absent" (capability-first, §3.5).
    #[error("denied")]
    Denied,
    /// A referenced reminder was not found in this workspace (a denied caller gets `Denied`,
    /// never `NotFound`, so this leaks nothing).
    #[error("not found")]
    NotFound,
    /// The cron schedule is not a valid 5-field expression, or its "next after T" could not be
    /// computed. Surfaced as `BadInput` at the MCP boundary so the author gets feedback.
    #[error("bad cron: {0}")]
    BadCron(String),
    /// The input was not valid (missing field, bad action shape, …).
    #[error("bad input: {0}")]
    BadInput(String),
    /// A durable store operation failed.
    #[error("store error: {0}")]
    Store(#[from] lb_store::StoreError),
}
