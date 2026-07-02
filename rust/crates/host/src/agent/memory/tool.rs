//! The MCP bridge for `agent.memory.*` (agent-memory scope MCP surface) — the four verbs over the
//! one MCP contract (`lb call agent.memory.<verb>` / `POST /mcp/call`). `list`/`get` read, `set`
//! upserts, `delete` removes. No live feed (memory is state, read at session start) and no batch
//! (facts are written one at a time by design). Each verb runs its own gate inside `verbs.rs`.

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

use super::model::Memory;
use super::verbs::{memory_delete, memory_get, memory_list, memory_set};

/// Dispatch an `agent.memory.<verb>` call. Returns `Some(result)` for a memory verb, `None` for a
/// verb outside this surface (so the `agent.` dispatcher can fall through). Arg parsing + the gate
/// live in `verbs.rs`; this is shape-in / shape-out only.
pub async fn call_agent_memory_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Option<Result<Value, ToolError>> {
    match qualified_tool {
        "agent.memory.list" => Some(list(store, principal, ws).await),
        "agent.memory.get" => Some(get(store, principal, ws, input).await),
        "agent.memory.set" => Some(set(store, principal, ws, input).await),
        "agent.memory.delete" => Some(delete(store, principal, ws, input).await),
        _ => None,
    }
}

async fn list(store: &Store, principal: &Principal, ws: &str) -> Result<Value, ToolError> {
    let rows = memory_list(store, principal, ws).await?;
    // The derived index rows (slug + description + kind + scope + updated) — the body is loaded on
    // demand by `get`, never in the list (keep the index cheap).
    Ok(json!({
        "memories": rows.iter().map(row_index).collect::<Vec<_>>()
    }))
}

async fn get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let slug = str_arg(input, "slug")?;
    let scope = input.get("scope").and_then(|v| v.as_str());
    match memory_get(store, principal, ws, scope, slug).await? {
        Some(m) => Ok(row_full(&m)),
        None => Err(ToolError::NotFound),
    }
}

async fn set(
    store: &Store,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let scope = input.get("scope").and_then(|v| v.as_str());
    let m = memory_set(
        store,
        principal,
        ws,
        scope,
        str_arg(input, "slug")?,
        str_arg(input, "description")?,
        str_arg(input, "kind")?,
        str_arg(input, "body")?,
        u64_arg(input, "ts")?,
    )
    .await?;
    Ok(json!({ "scope": m.scope, "slug": m.slug }))
}

async fn delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let scope = input.get("scope").and_then(|v| v.as_str());
    memory_delete(store, principal, ws, scope, str_arg(input, "slug")?).await?;
    Ok(json!({ "ok": true }))
}

/// One index row — slug, description, kind, scope, updated (no body).
fn row_index(m: &Memory) -> Value {
    json!({
        "scope": m.scope,
        "slug": m.slug,
        "description": m.description,
        "kind": m.kind.as_str(),
        "updated_at": m.updated_at,
        "updated_by": m.updated_by,
    })
}

/// The full fact (index row + body) — the `get` result.
fn row_full(m: &Memory) -> Value {
    json!({
        "scope": m.scope,
        "slug": m.slug,
        "description": m.description,
        "kind": m.kind.as_str(),
        "body": m.body,
        "updated_at": m.updated_at,
        "updated_by": m.updated_by,
    })
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing string arg: {key}")))
}

fn u64_arg(input: &Value, key: &str) -> Result<u64, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_u64())
        .ok_or_else(|| ToolError::BadInput(format!("missing u64 arg: {key}")))
}
