//! The `workspace_agent_config:[ws]` SCHEMAFULL table + its raw get/set store verbs (agent-config
//! scope). One deterministic record per workspace (composite id `[ws]`) → an offline edit UPSERTs
//! idempotently on replay (LWW), exactly like `workspace_prefs`. Namespace-scoped (the hard wall);
//! the id's `ws` element is denormalized for query convenience. Raw verbs: they run *after* the host
//! capability gate — this file does no authorization.

use lb_store::{Store, StoreError};
use serde_json::{json, Value};

use super::model::AgentConfig;

/// The per-workspace agent-config table.
pub const AGENT_CONFIG_TABLE: &str = "workspace_agent_config";

/// The columns to project on a read — explicitly NOT `id` (a composite RecordId whose array id-part
/// does not round-trip cleanly through `serde_json::Value`), only the config fields.
/// NB: `active_persona` is deliberately NOT projected — legacy decode-only (persona-session #5); the
/// boot migration reads it directly, nothing else may.
const AGENT_CONFIG_COLUMNS: &str =
    "default_runtime, model_endpoint, active_definition, enabled_personas, compact_budget, \
     loop_window, exfiltration_guard";

/// Define the `workspace_agent_config` table in `ws`. Idempotent (`DEFINE ... IF NOT EXISTS`).
/// SCHEMAFULL with `default_runtime` nullable and `model_endpoint` a flexible object (the nested
/// names-only endpoint, validated by serde on the way in, not by DB asserts).
pub async fn define_agent_config_schema(store: &Store, ws: &str) -> Result<(), StoreError> {
    let sql = format!(
        "DEFINE TABLE IF NOT EXISTS {AGENT_CONFIG_TABLE} SCHEMAFULL;
         DEFINE FIELD IF NOT EXISTS ws ON {AGENT_CONFIG_TABLE} TYPE string;
         DEFINE FIELD IF NOT EXISTS default_runtime ON {AGENT_CONFIG_TABLE} TYPE option<string>;
         DEFINE FIELD IF NOT EXISTS model_endpoint ON {AGENT_CONFIG_TABLE} FLEXIBLE TYPE option<object>;
         DEFINE FIELD IF NOT EXISTS active_definition ON {AGENT_CONFIG_TABLE} TYPE option<string>;
         DEFINE FIELD IF NOT EXISTS active_persona ON {AGENT_CONFIG_TABLE} TYPE option<string>;
         DEFINE FIELD IF NOT EXISTS enabled_personas ON {AGENT_CONFIG_TABLE} TYPE option<array<string>>;
         DEFINE FIELD IF NOT EXISTS compact_budget ON {AGENT_CONFIG_TABLE} TYPE option<number>;
         DEFINE FIELD IF NOT EXISTS loop_window ON {AGENT_CONFIG_TABLE} TYPE option<number>;
         DEFINE FIELD IF NOT EXISTS exfiltration_guard ON {AGENT_CONFIG_TABLE} TYPE option<bool>;"
    );
    store.query_ws(ws, &sql, vec![]).await?;
    Ok(())
}

/// Load `ws`'s agent config. `Ok(None)` when no config has been set (the workspace inherits the
/// node's compiled-in default runtime).
pub async fn get_agent_config(store: &Store, ws: &str) -> Result<Option<AgentConfig>, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!(
                "SELECT {AGENT_CONFIG_COLUMNS} FROM type::thing('{AGENT_CONFIG_TABLE}', [$ws])"
            ),
            vec![("ws".into(), Value::String(ws.to_string()))],
        )
        .await?;
    let rows: Vec<Value> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    match rows.into_iter().next() {
        None => Ok(None),
        Some(row) => {
            let cfg: AgentConfig =
                serde_json::from_value(row).map_err(|e| StoreError::Decode(e.to_string()))?;
            Ok(Some(cfg))
        }
    }
}

/// Apply `patch` to `ws`'s agent-config record, creating it if absent. Present fields overwrite
/// (same-field LWW); absent fields (filtered by `skip_serializing_if`) stay untouched.
pub async fn set_agent_config(
    store: &Store,
    ws: &str,
    patch: &AgentConfig,
) -> Result<(), StoreError> {
    define_agent_config_schema(store, ws).await?;
    let mut merge =
        match serde_json::to_value(patch).map_err(|e| StoreError::Decode(e.to_string()))? {
            Value::Object(map) => map,
            other => json!({ "_": other })
                .as_object()
                .cloned()
                .unwrap_or_default(),
        };
    merge.insert("ws".into(), Value::String(ws.to_string()));

    store
        .query_ws(
            ws,
            &format!("UPSERT type::thing('{AGENT_CONFIG_TABLE}', [$ws]) MERGE $patch"),
            vec![
                ("ws".into(), Value::String(ws.to_string())),
                ("patch".into(), Value::Object(merge)),
            ],
        )
        .await?;
    Ok(())
}
