//! Upsert the workspace-default `workspace_prefs:[ws]` record from a patch (prefs scope
//! `prefs.set_default`, **admin-gated** at the host). Same MERGE semantics as a user set; a single
//! deterministic record per workspace.

use lb_store::{Store, StoreError};
use serde_json::Value;

use super::schema::{define_prefs_schema, WORKSPACE_PREFS_TABLE};
use super::set::patch_object;
use crate::prefs::Prefs;

/// Apply `patch` to the workspace-default record for `ws`, creating it if absent.
pub async fn set_workspace_prefs(store: &Store, ws: &str, patch: &Prefs) -> Result<(), StoreError> {
    define_prefs_schema(store, ws).await?;
    let mut merge = patch_object(patch)?;
    merge.insert("ws".into(), Value::String(ws.to_string()));

    store
        .query_ws(
            ws,
            &format!("UPSERT type::thing('{WORKSPACE_PREFS_TABLE}', [$ws]) MERGE $patch"),
            vec![
                ("ws".into(), Value::String(ws.to_string())),
                ("patch".into(), Value::Object(merge)),
            ],
        )
        .await?;
    Ok(())
}
