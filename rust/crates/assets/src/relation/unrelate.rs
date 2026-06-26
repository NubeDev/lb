//! Delete a relation edge — the revoke verb. Removing the `share`/`grant`/`member` edge makes
//! the asset instantly invisible again on the next guarded read (sharing is a live relation,
//! not a content copy — files scope).
//!
//! The store has no delete verb yet, so revoke is an upsert-to-empty: the row is overwritten
//! with a tombstone `data` and `related` is taught to treat a tombstone as absent. Keeping the
//! tombstone (rather than a hard delete) is also the §6.8-friendly choice — a delete is an
//! append-style state change that syncs idempotently, not a row that vanishes under a peer.

use lb_store::{write, Store, StoreError};

use super::{rel_id, TABLE};

/// The marker a revoked edge carries. `related` checks for it so a tombstoned edge reads as
/// absent without a store-level delete.
pub(crate) const TOMBSTONE: &str = "__revoked__";

/// Revoke the edge `a -[kind]-> b` in workspace `ws`. Idempotent; a never-created edge revokes
/// to the same tombstone harmlessly.
pub async fn unrelate(
    store: &Store,
    ws: &str,
    kind: &str,
    a: &str,
    b: &str,
) -> Result<(), StoreError> {
    let tombstone = serde_json::json!({ "kind": TOMBSTONE });
    write(store, ws, TABLE, &rel_id(kind, a, b), &tombstone).await
}
