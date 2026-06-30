//! List every live `a` for a given `(kind, b)` — the **inverse** of `list_related`. The
//! document-store use is **backlinks**: every doc that links *to* a target doc, i.e. every
//! `a` where `a -[doclink]-> b`. Backed by the denormalized `bpair` filter (`{kind}__{b}`),
//! so it is one namespace-scoped `store::list`. Tombstoned (revoked) edges are skipped.

use lb_store::{list as store_list, Store, StoreError};

use super::model::Relation;
use super::unrelate::TOMBSTONE;
use super::TABLE;

/// Return every live `a` such that `a -[kind]-> b` in workspace `ws`. Empty if none — never
/// another workspace's edges. Order is unspecified (the caller uses it as a set).
pub async fn list_related_inverse(
    store: &Store,
    ws: &str,
    kind: &str,
    b: &str,
) -> Result<Vec<String>, StoreError> {
    let bpair = format!("{kind}__{b}");
    let rows = store_list(store, ws, TABLE, "bpair", &bpair).await?;
    let mut out = Vec::new();
    for v in rows {
        if v.get("kind").and_then(|k| k.as_str()) == Some(TOMBSTONE) {
            continue;
        }
        let rel: Relation =
            serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?;
        out.push(rel.a);
    }
    Ok(out)
}
