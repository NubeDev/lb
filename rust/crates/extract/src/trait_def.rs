//! The `Extractor` trait — one per-mime parser, the whole contract of this crate (doc-extraction
//! scope). Pure and deterministic: given the SAME `(bytes, mime, opts)` it returns the SAME
//! `Vec<ExtractedDoc>`, with no network, no clock, no randomness. That determinism is what makes
//! the ledger's idempotency real (re-run with the same checksum + version is a no-op) and the
//! snapshot tests a fidelity contract.

use crate::error::ExtractError;
use crate::model::{ExtractOpts, ExtractedDoc};

/// A per-mime extractor. One impl per mime family, one file each (FILE-LAYOUT). An extractor is
/// selected by [`extractor_for`](crate::extractor_for); the host never constructs one directly.
pub trait Extractor: Send + Sync {
    /// A stable id recorded in the extraction ledger (`{extractor_id, extractor_version}`), so a
    /// re-derivation can tell "same extractor, same version → no-op" from "version bumped →
    /// re-derive". Kebab-case, e.g. `"pdf-text"`, `"xlsx"`, `"csv"`, `"html"`, `"text"`.
    fn id(&self) -> &'static str;

    /// The extractor version. Bumping it is the explicit signal to re-derive a corpus in place
    /// (scope: "extractor version churn is a derived-data migration; silent upgrades are
    /// forbidden"). Start at 1; bump when the output for the same bytes meaningfully changes.
    fn version(&self) -> u32;

    /// Turn `bytes` (of the given `mime`) into one or more markdown docs. Deterministic and pure.
    /// - `Ok(vec)` — one or more `ExtractedDoc`s (never empty on success; if the source is
    ///   genuinely empty an extractor returns one doc with an empty/whitespace body, so the caller
    ///   still gets provenance — the host decides whether to keep it).
    /// - `Err(Unsupported)` — the mime is claimed but this exact input is a v1 non-goal.
    /// - `Err(Failed)` — the bytes are corrupt/truncated/malformed.
    fn extract(
        &self,
        bytes: &[u8],
        mime: &str,
        opts: &ExtractOpts,
    ) -> Result<Vec<ExtractedDoc>, ExtractError>;
}
