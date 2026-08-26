//! [`DecodeError`] — why a file could not become samples.
//!
//! Deliberately small, and deliberately **not** used for "some rows were bad". A file decode has two
//! genuinely different failure shapes and conflating them is how a data pipeline goes quietly wrong:
//!
//! - **The file is not what it claims** (`UnknownFormat`, `Malformed`) — nothing can be recovered,
//!   the caller must surface it. An error.
//! - **Row 4,102 of 4,320 had a blank where a number should be** — the other 4,319 rows are real
//!   data that a business wants. Those are [`warnings`](super::Decoded::warnings) on a successful
//!   decode, not an error, because failing the whole import over one bad cell is how a month of
//!   meter data gets thrown away.
//!
//! The rule: an error means *no* samples; a warning means *fewer* samples, and the caller can see
//! exactly how many and why.

/// Why a decode produced nothing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    /// No decoder is registered under this format id.
    #[error("unknown format '{0}'")]
    UnknownFormat(String),
    /// The bytes are not valid for the named format at all (wrong header, not text, empty).
    #[error("{format}: {reason}")]
    Malformed { format: String, reason: String },
}

impl DecodeError {
    pub fn malformed(format: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Malformed {
            format: format.into(),
            reason: reason.into(),
        }
    }
}
