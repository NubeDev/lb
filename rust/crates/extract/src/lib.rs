//! `lb-extract` — the pure per-mime extraction layer (doc-extraction scope).
//!
//! Given binary bytes and a mime type, derive one or more markdown [`ExtractedDoc`]s. This crate is
//! deliberately **host-free**: no store, no auth, no bus, no network, no clock, no randomness —
//! only `bytes → markdown`. That is what lets the host wrap it in the capability chokepoint + the
//! `docs.extract` job + the provenance ledger without this crate knowing any of it exists, and what
//! makes every extractor fixture-testable offline (the fidelity contract is a snapshot test).
//!
//! The surface is tiny:
//!   - [`Extractor`] — the per-mime trait (`extract(bytes, mime, opts) -> Vec<ExtractedDoc>`).
//!   - [`extractor_for`] — the mime→extractor registry (the ONE place that knows the mapping);
//!     `None` = the host's per-item `unsupported` outcome.
//!   - [`ExtractedDoc`] / [`ExtractOpts`] / [`SplitPolicy`] — the value types.
//!   - [`ExtractError`] — `Unsupported` (honest "we don't do this") vs `Failed` (corrupt input).
//!
//! Generic over mime, never a domain noun (rule 10): an extractor keys on `application/pdf`, never
//! on what the document is *about* — tags/title/visibility are the caller's business, applied by
//! the host, not here.

mod error;
mod extractor;
mod model;
mod registry;
mod trait_def;

pub use error::ExtractError;
pub use model::{ExtractOpts, ExtractedDoc, SplitPolicy};
pub use registry::{extractor_for, is_supported, MIME_XLSX};
pub use trait_def::Extractor;
