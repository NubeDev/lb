//! Create a relation edge, and test whether one exists — the write + existence verbs.
//!
//! `relate` is idempotent on `(kind, a, b)` (the same edge upserts the same row). `related` is
//! the membership question the host asks on every guarded read ("is this user in that team?",
//! "did the workspace grant that skill?"). Both are namespace-scoped (README §7) — raw verbs.

use lb_store::{read, write, Store, StoreError};

use super::model::Relation;
use super::unrelate::TOMBSTONE;
use super::{rel_id, TABLE};

/// Create (idempotently) the edge `a -[kind]-> b` in workspace `ws`.
pub async fn relate(
    store: &Store,
    ws: &str,
    kind: &str,
    a: &str,
    b: &str,
) -> Result<(), StoreError> {
    let rel = Relation::new(kind, a, b);
    let value = serde_json::to_value(&rel).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, &rel_id(kind, a, b), &value).await
}

/// Does a *live* edge `a -[kind]-> b` exist in workspace `ws`? `false` for another workspace's
/// edge (the namespace wall makes a cross-workspace edge invisible — README §7) and `false`
/// for a revoked (tombstoned) edge (`unrelate`).
pub async fn related(
    store: &Store,
    ws: &str,
    kind: &str,
    a: &str,
    b: &str,
) -> Result<bool, StoreError> {
    match read(store, ws, TABLE, &rel_id(kind, a, b)).await? {
        Some(value) => Ok(!is_tombstone(&value)),
        None => Ok(false),
    }
}

/// A revoked edge is a `{ "kind": "__revoked__" }` tombstone — read as absent.
fn is_tombstone(value: &serde_json::Value) -> bool {
    value.get("kind").and_then(|k| k.as_str()) == Some(TOMBSTONE)
}
