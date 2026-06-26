//! The **`tagged` provenance edge** — applying a tag is a `RELATE entity -> tagged -> tag` where the
//! edge is a *record* carrying provenance: `at`, `by` (principal), `source`, `confidence`, `expires`
//! (tags scope).
//!
//! **Edge identity is `(entity, tag, source)`** — so a human asserting `kind:telemetry` and an agent
//! later *inferring* the same coexist as TWO edges (both attributions preserved); a re-tag from the
//! SAME source upserts in place (idempotent, `by`/`confidence`/`expires` mutable). Keying on
//! `(entity, tag)` alone is REJECTED — it would let the AI write overwrite the human's `by`/
//! `confidence`, silently breaking the distinguish-by-source goal.

use serde::{Deserialize, Serialize};

/// The `tagged` edge (relation) table.
pub const TAGGED_TABLE: &str = "tagged";

/// Who/what asserted a tag — part of the edge identity, so different sources coexist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Human,
    Inferred,
    Producer,
    System,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Human => "human",
            Source::Inferred => "inferred",
            Source::Producer => "producer",
            Source::System => "system",
        }
    }
}

/// The provenance an edge carries. `confidence` defaults to 1.0 (a hard assertion); `expires` is an
/// optional logical expiry (caller-supplied, not wall-clock).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    /// Logical timestamp of the assertion (caller-injected — determinism §3, never wall-clock).
    pub at: u64,
    /// The asserting principal (`user:…`, `key:…`, `ext:…`).
    pub by: String,
    /// Who/what asserted it — part of the edge identity.
    pub source: Source,
    /// Confidence in `[0,1]`; 1.0 is a hard assertion.
    pub confidence: f64,
    /// Optional logical expiry.
    pub expires: Option<u64>,
}

impl Provenance {
    /// A hard human/system assertion at logical time `at`.
    pub fn new(at: u64, by: impl Into<String>, source: Source) -> Self {
        Self {
            at,
            by: by.into(),
            source,
            confidence: 1.0,
            expires: None,
        }
    }
}
