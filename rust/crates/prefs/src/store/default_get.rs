//! Read the workspace-default `workspace_prefs:[ws]` record — the second link in the resolution
//! chain (admins set it; users inherit it). `None` when unset. Namespace-scoped; raw verb.

use lb_store::{Store, StoreError};
use serde_json::Value;

use super::get::decode_first;
use super::schema::{PREFS_COLUMNS, WORKSPACE_PREFS_TABLE};
use crate::prefs::Prefs;

/// Load the workspace-default prefs for `ws`. `Ok(None)` when no default has been set.
pub async fn get_workspace_prefs(store: &Store, ws: &str) -> Result<Option<Prefs>, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!("SELECT {PREFS_COLUMNS} FROM type::thing('{WORKSPACE_PREFS_TABLE}', [$ws])"),
            vec![("ws".into(), Value::String(ws.to_string()))],
        )
        .await?;
    let rows: Vec<Value> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    decode_first(rows)
}
