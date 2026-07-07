//! `get` — read one insight by id (insights umbrella scope).
//!
//! The store read is namespace-scoped, so a get for workspace A can only ever read A's insights
//! (README §7). Authorization is the host layer's job — this is the raw verb, run *after*
//! `caps::check` (workspace-first §7, then `mcp:insight.get:call`).

use lb_store::{read, Store, StoreError};

use crate::insight::Insight;
use crate::insight::OCC_TABLE;
use crate::insight_id::record_id;

/// Return the insight at `(ws, id)`, or `None` if absent in this workspace.
pub async fn get(store: &Store, ws: &str, id: &str) -> Result<Option<Insight>, StoreError> {
    let Some(value) = read(store, ws, OCC_TABLE, &record_id(id)).await? else {
        return Ok(None);
    };
    let insight: Insight =
        serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(Some(insight))
}
