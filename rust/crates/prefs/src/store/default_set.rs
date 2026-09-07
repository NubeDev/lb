//! Upsert the workspace-default `workspace_prefs:[ws]` record from a patch (prefs scope
//! `prefs.set_default`, **admin-gated** at the host). Same MERGE semantics as a user set; a single
//! deterministic record per workspace.

use lb_store::{Store, StoreError};
use serde_json::Value;

use super::schema::{define_prefs_schema, WORKSPACE_PREFS_TABLE};
use super::set::patch_object;
use crate::clear::{clear_set_clause, PrefsAxis};
use crate::prefs::Prefs;

/// Apply `patch` to the workspace-default record for `ws`, creating it if absent. Axes named in
/// `clear` are set back to NULL, so the workspace stops shipping that axis and members fall through
/// to the built-in fallback (the same semantics as a user clear, one link down).
pub async fn set_workspace_prefs(
    store: &Store,
    ws: &str,
    patch: &Prefs,
    clear: &[PrefsAxis],
) -> Result<(), StoreError> {
    define_prefs_schema(store, ws).await?;
    let mut merge = patch_object(patch)?;
    merge.insert("ws".into(), Value::String(ws.to_string()));

    store
        .query_ws(
            ws,
            &{
                // A clear must be SurrealQL, not a JSON null in the merge -- see
                // `clear::clear_set_clause`. MERGE first so the patch lands, then SET the cleared
                // axes to NONE; both statements address the same record.
                let upsert = format!("UPSERT type::record('{WORKSPACE_PREFS_TABLE}', [$ws]) MERGE $patch");
                match clear_set_clause(clear) {
                    None => upsert,
                    Some(sets) => format!(
                        "{upsert};\nUPDATE type::record('{WORKSPACE_PREFS_TABLE}', [$ws]) SET {sets}"
                    ),
                }
            },
            vec![
                ("ws".into(), Value::String(ws.to_string())),
                ("patch".into(), Value::Object(merge)),
            ],
        )
        .await?;
    Ok(())
}
