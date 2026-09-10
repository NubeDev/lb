//! `get` — read one case by id (case-plane scope).
//!
//! The store read is namespace-scoped, so a get for workspace A can only ever read A's cases
//! (README §7). Authorization is the host layer's job — this is the raw verb, run *after*
//! `caps::check` (workspace-first §7, then `mcp:case.get:call`).

use lb_store::{read, Store, StoreError};

use crate::case::{Case, TABLE};

/// Return the case at `(ws, id)`, or `None` if absent in this workspace.
pub async fn get(store: &Store, ws: &str, id: &str) -> Result<Option<Case>, StoreError> {
    let Some(value) = read(store, ws, TABLE, id).await? else {
        return Ok(None);
    };
    let case: Case =
        serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(Some(case))
}
