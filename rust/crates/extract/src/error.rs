//! The extraction error — the honest failure vocabulary an [`Extractor`](crate::Extractor)
//! returns (doc-extraction scope). Two shapes matter to the host's per-item result:
//!   - [`ExtractError::Unsupported`] — no extractor claims this mime (or the input is a shape v1
//!     refuses, e.g. an image-only PDF with no text layer). NEVER a silent empty doc — the caller
//!     is told plainly, so "we can't yet" is distinguishable from "there was nothing in it".
//!   - [`ExtractError::Failed`] — an extractor claimed the mime but the bytes were corrupt /
//!     truncated / malformed. Carries a reason for the ledger's per-item `failed(reason)`.

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExtractError {
    /// No registered extractor handles this mime, or the extractor recognizes the mime but the
    /// content is a variant v1 does not do (image-only PDF → OCR is a non-goal). The host maps
    /// this to the per-item `unsupported` outcome — never an empty derived doc.
    #[error("unsupported: {0}")]
    Unsupported(String),
    /// The extractor claimed the mime but the bytes could not be parsed (corrupt/truncated/
    /// malformed). The reason rides the ledger's per-item `failed` outcome; the source stays the
    /// truth and can be re-extracted after a parser fix.
    #[error("extraction failed: {0}")]
    Failed(String),
}

impl ExtractError {
    /// Convenience: an `Unsupported` from any displayable reason.
    pub fn unsupported(reason: impl Into<String>) -> Self {
        ExtractError::Unsupported(reason.into())
    }

    /// Convenience: a `Failed` from any displayable reason.
    pub fn failed(reason: impl Into<String>) -> Self {
        ExtractError::Failed(reason.into())
    }
}
