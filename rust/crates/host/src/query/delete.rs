//! `query.delete {id}` — tombstone-upsert a saved query (idempotent, sync-safe). A delete of an
//! absent/already-deleted query is a no-op, not an error. Gated `mcp:query.delete:call` at the bridge.

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize;
use super::error::QueryError;
use super::record::{put, SavedQuery};

/// Soft-delete query `id` in `ws` (sets `removed = true`, bumps `ts`). Idempotent.
pub async fn query_delete(
    store: &Store,
    caller: &Principal,
    ws: &str,
    id: &str,
    ts: u64,
) -> Result<(), QueryError> {
    authorize(caller, ws, "query.delete")?;
    // Resolve existing (preserve its fields on the tombstone, audit) or synthesize a minimal one so
    // the delete is idempotent even for a never-saved id (mirrors `datasource_remove`).
    let mut q = match super::record::resolve(store, ws, id).await? {
        Some(q) => q,
        None => SavedQuery::new(id, id, "", "prql", "", "platform", Vec::new(), ts),
    };
    if q.removed {
        return Ok(());
    }
    q.removed = true;
    q.ts = ts;
    put(store, ws, &q).await?;
    Ok(())
}
