//! The extraction value types — the pure input/output shapes of an [`Extractor`](crate::Extractor)
//! (doc-extraction scope). No host types here (no doc id, no workspace, no principal): an extractor
//! knows *bytes → markdown parts*, and the host maps those parts onto docs + edges. That split is
//! what keeps this crate host-free and fixture-testable offline.

use serde::{Deserialize, Serialize};

/// One markdown document derived from a source, before the host assigns it a doc id. A single
/// source may yield several (a workbook's sheets under [`SplitPolicy::PerPart`]); each carries a
/// stable [`part`](ExtractedDoc::part) key so re-derivation lands on the SAME doc id (the host's
/// stable-identity rule — links + embeddings migrate instead of orphaning).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedDoc {
    /// A human title suggestion (a sheet name, an HTML `<title>`, the first heading). The host may
    /// override it with the caller-supplied title; this is only the extractor's best guess.
    pub title_hint: String,
    /// The derived body — always markdown (the one output contract; PDF/XLSX/HTML all normalize to
    /// it, so the downstream link-graph + embeddings pipeline see one shape).
    pub markdown: String,
    /// The stable within-source part key. `None` = the whole source is one doc. `Some(key)` names
    /// this part (e.g. a sheet name) so the host derives a stable per-part doc id. Deterministic:
    /// the SAME bytes always produce the SAME part keys in the SAME order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<String>,
}

impl ExtractedDoc {
    /// A whole-source doc (no part key).
    pub fn whole(title_hint: impl Into<String>, markdown: impl Into<String>) -> Self {
        Self {
            title_hint: title_hint.into(),
            markdown: markdown.into(),
            part: None,
        }
    }

    /// A named part of a multi-part source (its `part` key is stable across re-derivation).
    pub fn part(
        title_hint: impl Into<String>,
        markdown: impl Into<String>,
        part: impl Into<String>,
    ) -> Self {
        Self {
            title_hint: title_hint.into(),
            markdown: markdown.into(),
            part: Some(part.into()),
        }
    }
}

/// How a multi-part source (a workbook) is turned into docs. The caller chooses; extractors that
/// have no notion of parts (PDF, HTML, text) ignore it and always return one doc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SplitPolicy {
    /// One doc for the whole source; parts become sections under headings (the default — a
    /// workbook reads as one document, chosen from the real multi-sheet fixture: a reader wants
    /// the whole book in one place, and per-sheet docs fragment search/backlinks needlessly).
    #[default]
    Whole,
    /// One doc per part (per sheet). Each gets a stable `part` key + its own derived doc id.
    PerPart,
}

/// Caller options threaded into every [`extract`](crate::Extractor::extract) call. Deliberately
/// small in v1; the seam is where model-assisted / OCR extractors will read budget + hints later
/// (scope non-goals), so it is an owned struct, not a bare enum, to stay additive.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExtractOpts {
    /// Multi-part split behavior (workbook → one doc or per-sheet docs).
    pub split: SplitPolicy,
    /// The max cells a single embedded table inlines before it is summarized + truncated (a 10k-row
    /// sheet must not inline whole — scope risk "size caps for embedded tables"). `0` means the
    /// extractor's built-in default.
    pub max_table_cells: usize,
}
