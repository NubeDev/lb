//! The on-disk envelope for a stored value. The host speaks `serde_json::Value`; SurrealDB
//! speaks its own value model. We bridge by wrapping the host JSON under a single concrete
//! `data` field — a typed shape SurrealDB serializes/deserializes cleanly, avoiding the
//! `serde_json::Value` ↔ SurrealDB enum-tag mismatch (debugging/store/
//! content-rejects-serde-json-value.md).
//!
//! Every record also carries a store-managed monotonic `rev` (revision), bumped on every write
//! (see `write.rs`/`write_tx.rs`). It is the optimistic-concurrency token the undo journal's
//! *conditional restore* tests against (`docs/scope/undo/undo-scope.md`): an undo only applies
//! when the live record's `rev` still equals the `rev` the undo expects to overwrite. `rev` is
//! invisible to the plain [`read`](crate::read) path (which still returns just the host `data`),
//! and surfaced explicitly via [`read_versioned`](crate::read_versioned).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The very first revision a record gets on its first write. A fresh write of a never-seen id
/// lands at `rev = 1`; absence is `rev = 0` (see [`Versioned::ABSENT_REV`]).
pub const FIRST_REV: u64 = 1;

/// A stored document: the host's JSON value under `data`, plus the store-managed `rev`. The
/// record `id` lives outside this struct (SurrealDB manages it), so deserialization never trips
/// on the `id` Thing type.
#[derive(Serialize, Deserialize)]
pub(crate) struct Record {
    pub data: Value,
    /// Store-managed monotonic revision. Defaults to [`FIRST_REV`] for records written before
    /// `rev` existed (forward-compatible read of legacy rows).
    #[serde(default = "default_rev")]
    pub rev: u64,
}

fn default_rev() -> u64 {
    FIRST_REV
}

/// A record read together with its `rev` — the unit the conditional-restore predicate works on.
/// `value: None` means the record is absent in this namespace, in which case `rev` is
/// [`Versioned::ABSENT_REV`] (0). Absence is a first-class state: a *create* undo must be able to
/// say "I expect this id to still be absent" and a *delete* undo to restore an absent record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Versioned {
    pub value: Option<Value>,
    pub rev: u64,
}

impl Versioned {
    /// The `rev` reported for an absent record. Distinct from any real write (`rev >= FIRST_REV`),
    /// so "expect absent" (rev 0) never collides with "expect a written value".
    pub const ABSENT_REV: u64 = 0;

    /// An absent record at the well-known absence revision.
    pub fn absent() -> Self {
        Self {
            value: None,
            rev: Self::ABSENT_REV,
        }
    }
}
