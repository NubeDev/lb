//! The extraction service value types (doc-extraction scope): the caller request, the per-item
//! outcome, and the durable **extraction ledger** record that is the provenance + idempotency
//! truth. Generic over mime, never a domain noun (rule 10): `tags`/`title`/`visibility` are
//! caller-supplied and land on the derived docs verbatim; the service invents no metadata.

use serde::{Deserialize, Serialize};

/// The store table the extraction ledger lives in (workspace-namespaced like everything else).
pub const EXTRACTION_TABLE: &str = "extraction";

/// The relation kind for the derivation edge: `derived_doc -[derived_from]-> source_media`. One
/// hop to "show me the original"; deleting the media flags (not silently breaks) its derived docs.
pub const DERIVED_FROM: &str = "derived_from";

/// A `docs.extract` request — the caller's input, normalized off the JSON. `media` is the list of
/// source media ids; the doc fields ride along and are applied to every derived doc.
#[derive(Debug, Clone)]
pub struct ExtractRequest {
    /// The source media ids to extract, in one workspace. Each yields 0..N derived docs.
    pub media: Vec<String>,
    /// Caller title override for the derived doc(s). Empty = use the extractor's title hint.
    pub title: Option<String>,
    /// Caller tags applied to every derived doc (the caller's business — rule 10).
    pub tags: Vec<String>,
    /// Per-part split policy (workbook → one doc or one per sheet).
    pub split: lb_extract::SplitPolicy,
    /// Force re-derivation at/under this extractor version even if the ledger already has a
    /// derivation for `(checksum, version)`. `None` = normal idempotent run (ledger hit → no-op).
    pub force_version: Option<u32>,
}

/// The outcome of extracting ONE source media id — the per-item result surface the scope requires
/// (`extracted | unsupported | failed(reason) | denied`). Serialized into the job payload + echoed
/// in the verb response, and derivable from the ledger for a durable read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ItemOutcome {
    /// One or more docs were derived (or the ledger already had them → idempotent no-op with the
    /// same doc ids). `reused` distinguishes a fresh derivation from a ledger hit.
    Extracted {
        media_id: String,
        doc_ids: Vec<String>,
        reused: bool,
    },
    /// No extractor claims the media's mime, or the input is a v1 non-goal (image-only PDF). Never
    /// an empty derived doc — an honest "we don't do this".
    Unsupported { media_id: String, reason: String },
    /// The extractor claimed the mime but the bytes were corrupt/truncated/malformed (or the
    /// extractor panicked — contained to this item). The source stays the truth; re-run after a fix.
    Failed { media_id: String, reason: String },
    /// The caller cannot read this source media (missing read reach, or it is another workspace's /
    /// nonexistent id — indistinguishable, no existence leak). The other items still extract.
    Denied { media_id: String },
}

impl ItemOutcome {
    /// The media id this outcome is about (for correlating results to inputs).
    pub fn media_id(&self) -> &str {
        match self {
            ItemOutcome::Extracted { media_id, .. }
            | ItemOutcome::Unsupported { media_id, .. }
            | ItemOutcome::Failed { media_id, .. }
            | ItemOutcome::Denied { media_id } => media_id,
        }
    }
}

/// One durable derivation record — the provenance + idempotency truth (scope: "an `extraction`
/// record per derivation"). Addressed by a stable id `{media_id}:{extractor_id}` so a re-run with
/// the SAME `(media_checksum, extractor_version)` reads it back and no-ops, and a version bump
/// overwrites it (re-derives into the SAME doc ids). Workspace-namespaced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Extraction {
    /// `{media_id}:{extractor_id}` — one derivation lineage per (source, extractor family).
    pub id: String,
    pub media_id: String,
    /// The source bytes' checksum at derivation time — the idempotency key half. A changed source
    /// (new checksum) is a new derivation even at the same extractor version.
    pub media_checksum: String,
    pub extractor_id: String,
    pub extractor_version: u32,
    /// The derived doc ids (stable across re-derivation — links + embeddings migrate in place).
    pub doc_ids: Vec<String>,
    /// Caller-injected logical timestamp (no wall-clock — testing §3 determinism).
    pub ts: u64,
}

impl Extraction {
    /// The stable ledger id for a `(media, extractor family)` lineage.
    pub fn make_id(media_id: &str, extractor_id: &str) -> String {
        format!("{media_id}:{extractor_id}")
    }
}
