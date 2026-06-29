//! `query.get {id}` / `query.list {}` — workspace-scoped reads of saved queries (query scope). Gated
//! at the bridge (`mcp:query.get`/`mcp:query.list`). `query.get` returns the full record (for re-
//! opening in the editor); `query.list` returns a flat roster (`id, name, target, lang, ts`) with NO
//! result data and NO query text beyond what the roster needs. A ws-B caller reads only ws-B queries
//! (the namespace wall) — the mandatory isolation property.

use lb_auth::Principal;
use lb_store::Store;
use serde::Serialize;

use super::authorize::authorize;
use super::error::QueryError;
use super::record::{query_tag, SavedQuery, TABLE};

/// Read one saved query by id. `NotFound` if absent or tombstoned (which is what a cross-tenant id
/// resolves to).
pub async fn query_get(
    store: &Store,
    caller: &Principal,
    ws: &str,
    id: &str,
) -> Result<SavedQuery, QueryError> {
    authorize(caller, ws, "query.get")?;
    super::record::resolve(store, ws, id)
        .await?
        .ok_or(QueryError::NotFound)
}

/// A roster row — the minimum a list needs (no text, no result data). Folders/tags are a follow-on.
#[derive(Debug, Clone, Serialize)]
pub struct QuerySummary {
    pub id: String,
    pub name: String,
    pub target: String,
    pub lang: String,
    pub ts: u64,
}

/// List saved queries in the workspace — a flat roster. Tombstones are skipped.
pub async fn query_list(
    store: &Store,
    caller: &Principal,
    ws: &str,
) -> Result<Vec<QuerySummary>, QueryError> {
    authorize(caller, ws, "query.list")?;
    let rows = lb_store::list(store, ws, TABLE, "tag", &query_tag()).await?;
    let mut out = Vec::new();
    for value in rows {
        let q: SavedQuery = serde_json::from_value(value)
            .map_err(|e| lb_store::StoreError::Decode(e.to_string()))?;
        if q.removed {
            continue;
        }
        out.push(QuerySummary {
            id: q.id,
            name: q.name,
            target: q.target,
            lang: q.lang,
            ts: q.ts,
        });
    }
    // Stable order by id so the roster is deterministic (no wall-clock in core, testing §3).
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}
