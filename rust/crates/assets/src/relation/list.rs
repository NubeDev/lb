//! List every live `b` for a given `(kind, a)` — e.g. every team a doc is shared to, every
//! channel it is linked into. Backed by the denormalized `pair` filter (`{kind}__{a}`), so it
//! is one namespace-scoped `store::list` (README §7). Tombstoned (revoked) edges are skipped.

use lb_store::{list as store_list, Store, StoreError};

use super::model::Relation;
use super::unrelate::TOMBSTONE;
use super::TABLE;

/// Return every live `b` such that `a -[kind]-> b` in workspace `ws`. Empty if none — never
/// another workspace's edges. Order is unspecified (the caller uses it as a set).
pub async fn list_related(
    store: &Store,
    ws: &str,
    kind: &str,
    a: &str,
) -> Result<Vec<String>, StoreError> {
    let pair = format!("{kind}__{a}");
    let rows = store_list(store, ws, TABLE, "pair", &pair).await?;
    let mut out = Vec::new();
    for v in rows {
        // A tombstone row has no `pair` (it was overwritten), so it won't match the filter —
        // but guard anyway against a future partial-tombstone shape.
        if v.get("kind").and_then(|k| k.as_str()) == Some(TOMBSTONE) {
            continue;
        }
        let rel: Relation =
            serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?;
        out.push(rel.b);
    }
    Ok(out)
}
