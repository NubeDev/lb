//! List every skill the workspace has granted — the catalog source for model-activated skills
//! (agent-run scope Part 5). A grant is the edge `grant: {skill_id} -> "ws"` (see `mod.rs`); the
//! workspace's granted *set* is "every `a` where `kind = grant`", which the store answers as one
//! field-equality filter on `kind`.
//!
//! Why a dedicated verb and not `list_related`: `list_related(kind, a)` lists every `b` for a
//! *known* `a`, but the catalog asks the inverse — every granted skill id, without knowing them in
//! advance. The `grant` kind is exclusively skill→ws (see the relation table in `mod.rs`), so a
//! filter on `kind = "grant"` returns exactly the granted skill ids (the `a` of each live edge).
//! Tombstoned (revoked) edges are skipped, so a revoked grant disappears from the catalog
//! immediately. Workspace-namespaced like every list — never another workspace's grants (§7).

use lb_store::{list as store_list, Store, StoreError};

use super::model::Relation;
use super::unrelate::TOMBSTONE;
use super::TABLE;

/// The `kind` of a skill grant edge (kept here next to the query that depends on it being the
/// sole skill-grant kind).
const GRANT_KIND: &str = "grant";

/// Return every live granted skill id (the `a` of each `grant` edge) in workspace `ws`. Empty if
/// none — never another workspace's grants. Order is unspecified (the caller uses it as a set).
pub async fn list_skill_grants(store: &Store, ws: &str) -> Result<Vec<String>, StoreError> {
    let rows = store_list(store, ws, TABLE, "kind", GRANT_KIND).await?;
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
