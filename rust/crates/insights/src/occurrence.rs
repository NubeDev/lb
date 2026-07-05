//! `Occurrence` — one lite, size-capped row per raise in a per-insight capped ring
//! (insight-occurrences-scope.md).
//!
//! The ring is the **recent evidence** under an insight — the last N firings with their per-firing
//! delta (score, reading, txn ref). The parent's `count` is the LIFETIME truth (monotone); the
//! ring's stored rows may be fewer (`count` is allowed to exceed the ring size). `seq` is
//! host-assigned, monotone per insight; the ring evicts oldest-first.

use serde::{Deserialize, Serialize};

use crate::severity::Severity;

/// The store table occurrence rows live in. One table per workspace namespace; `insight_id` is a
/// `data` field (so the ring scan is a filter by parent, not a separate table per insight).
pub const TABLE: &str = "insight_occ";

/// The hard size cap on `data` serialized — enforced at raise, oversize rejects the whole raise
/// as `BadInput` (never silent truncation; the producer must slim its payload or store evidence
/// elsewhere and link it). The page-context 4 KB-reject precedent, tuned smaller for a lite row.
pub const MAX_DATA_BYTES: usize = 2 * 1024;

/// One firing of an insight. Lite by construction: the per-firing delta only — never a repeat of
/// the parent's title/origin/tags.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Occurrence {
    /// Monotone per-insight sequence (host-assigned). The ring orders newest-first by this.
    pub seq: u64,
    /// Logical timestamp of the raise (no wall-clock — testing §3).
    pub ts: u64,
    /// The severity THIS firing carried (the parent reflects the newest).
    pub severity: Severity,
    /// Opaque JSON delta — score, reading, txn ref, evidence link. ≤ `MAX_DATA_BYTES` serialized.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub data: serde_json::Value,
}
