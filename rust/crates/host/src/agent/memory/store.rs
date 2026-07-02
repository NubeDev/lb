//! The `agent_memory` SCHEMAFULL table + its raw store verbs (agent-memory scope: "one SCHEMAFULL
//! `agent_memory` table. State, not motion."). One record per `{scope, slug}` in a workspace
//! namespace (the composite id `[scope, slug]`), so an offline `set` UPSERTs idempotently on replay
//! (LWW per fact — the right merge for a correction). Mirrors `workspace_agent_config`'s composite-id
//! SCHEMAFULL pattern. Raw verbs: they run *after* the host capability gate — no authorization here.

use lb_store::{Store, StoreError};
use serde_json::{json, Value};

use super::model::{Memory, MemoryScope};

/// The workspace-scoped agent-memory table.
pub const MEMORY_TABLE: &str = "agent_memory";

/// The columns projected on a read — NOT `id` (a composite RecordId array id-part does not
/// round-trip cleanly through `serde_json::Value`), only the fact fields.
const MEMORY_COLUMNS: &str = "scope, slug, description, body, kind, updated_at, updated_by";

/// Define the `agent_memory` table in `ws`. Idempotent (`DEFINE ... IF NOT EXISTS`). SCHEMAFULL so
/// the shape is enforced at the store; `kind` is a string (the serde enum validates the value on the
/// way in), `updated_at` a number.
pub async fn define_memory_schema(store: &Store, ws: &str) -> Result<(), StoreError> {
    let sql = format!(
        "DEFINE TABLE IF NOT EXISTS {MEMORY_TABLE} SCHEMAFULL;
         DEFINE FIELD IF NOT EXISTS scope ON {MEMORY_TABLE} TYPE string;
         DEFINE FIELD IF NOT EXISTS slug ON {MEMORY_TABLE} TYPE string;
         DEFINE FIELD IF NOT EXISTS description ON {MEMORY_TABLE} TYPE string;
         DEFINE FIELD IF NOT EXISTS body ON {MEMORY_TABLE} TYPE string;
         DEFINE FIELD IF NOT EXISTS kind ON {MEMORY_TABLE} TYPE string;
         DEFINE FIELD IF NOT EXISTS updated_at ON {MEMORY_TABLE} TYPE number;
         DEFINE FIELD IF NOT EXISTS updated_by ON {MEMORY_TABLE} TYPE string;"
    );
    store.query_ws(ws, &sql, vec![]).await?;
    Ok(())
}

/// Upsert one fact at `{scope, slug}` in `ws` (create or replace — LWW). Idempotent on the composite
/// id, so a double-applied offline `set` writes once.
pub async fn upsert_memory(store: &Store, ws: &str, mem: &Memory) -> Result<(), StoreError> {
    define_memory_schema(store, ws).await?;
    let content = serde_json::to_value(mem).map_err(|e| StoreError::Decode(e.to_string()))?;
    store
        .query_ws(
            ws,
            &format!("UPSERT type::thing('{MEMORY_TABLE}', [$scope, $slug]) CONTENT $content"),
            vec![
                ("scope".into(), Value::String(mem.scope.clone())),
                ("slug".into(), Value::String(mem.slug.clone())),
                ("content".into(), content),
            ],
        )
        .await?;
    Ok(())
}

/// Read one fact at `{scope, slug}` in `ws`. `None` if absent.
pub async fn read_memory(
    store: &Store,
    ws: &str,
    scope: &MemoryScope,
    slug: &str,
) -> Result<Option<Memory>, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!(
                "SELECT {MEMORY_COLUMNS} FROM type::thing('{MEMORY_TABLE}', [$scope, $slug])"
            ),
            vec![
                ("scope".into(), Value::String(scope.key())),
                ("slug".into(), Value::String(slug.to_string())),
            ],
        )
        .await?;
    let rows: Vec<Value> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    match rows.into_iter().next() {
        None => Ok(None),
        Some(row) => Ok(Some(
            serde_json::from_value(row).map_err(|e| StoreError::Decode(e.to_string()))?,
        )),
    }
}

/// Delete one fact at `{scope, slug}` in `ws`. Idempotent (deleting an absent fact is a no-op).
pub async fn delete_memory(
    store: &Store,
    ws: &str,
    scope: &MemoryScope,
    slug: &str,
) -> Result<(), StoreError> {
    store
        .query_ws(
            ws,
            &format!("DELETE type::thing('{MEMORY_TABLE}', [$scope, $slug])"),
            vec![
                ("scope".into(), Value::String(scope.key())),
                ("slug".into(), Value::String(slug.to_string())),
            ],
        )
        .await?;
    Ok(())
}

/// List every fact in one of the given `scopes` in `ws`, ordered by `updated_at` DESC (newest first —
/// the injection cap keeps the most-recently-updated). The scopes are the resolver's output
/// (`workspace` + the caller's own `member:{user}`), so the query is walled to what the principal may
/// see: a member never lists another member's rows because that scope key is never passed here.
pub async fn list_memories(
    store: &Store,
    ws: &str,
    scopes: &[MemoryScope],
) -> Result<Vec<Memory>, StoreError> {
    let keys: Vec<Value> = scopes.iter().map(|s| Value::String(s.key())).collect();
    let mut resp = store
        .query_ws(
            ws,
            &format!(
                "SELECT {MEMORY_COLUMNS} FROM {MEMORY_TABLE} WHERE scope IN $scopes \
                 ORDER BY updated_at DESC"
            ),
            vec![("scopes".into(), json!(keys))],
        )
        .await?;
    let rows: Vec<Value> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    rows.into_iter()
        .map(|r| serde_json::from_value(r).map_err(|e| StoreError::Decode(e.to_string())))
        .collect()
}
