//! The MCP bridge for the generic store-mutation verbs — host-native tools under the one MCP contract
//! (README §6.5). An agent, the UI, or a native sidecar's `SidecarClient` callback all reach
//! `store.write`/`store.delete` the SAME way they reach any tool: a qualified call with JSON in/out.
//! Each verb authorizes the per-table gate first; denials are opaque (`ToolError::Denied`); a bad
//! argument surfaces as `BadInput`.
//!
//! Host-native — not a wasm extension, so NOT in the runtime `Registry`; the dispatcher routes
//! `store.write`/`store.delete` here (mirroring `store.query`/`store.schema`).

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

use super::error::StoreMutateError;
use super::run::{store_delete_run, store_write_run};

/// Dispatch a store-mutation MCP call. `input` is the verb's JSON arguments; the return is
/// `{ table, id }`. Each verb authorizes the per-table `store:<table>:write` gate first.
pub async fn call_store_mutate_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let table = str_arg(input, "table")?;
    let id = str_arg(input, "id")?;
    match qualified_tool {
        "store.write" => {
            let value = input
                .get("value")
                .ok_or_else(|| ToolError::BadInput("missing arg: value".into()))?;
            let (t, i) = store_write_run(store, principal, ws, table, id, value)
                .await
                .map_err(to_tool)?;
            Ok(json!({ "table": t, "id": i }))
        }
        "store.delete" => {
            let (t, i) = store_delete_run(store, principal, ws, table, id)
                .await
                .map_err(to_tool)?;
            Ok(json!({ "table": t, "id": i }))
        }
        _ => Err(ToolError::NotFound),
    }
}

/// Map the mutation gate's outcome onto the MCP tool error. `Denied` stays opaque; a bad argument is
/// author feedback (`BadInput`); a store fault is `Extension`.
fn to_tool(e: StoreMutateError) -> ToolError {
    match e {
        StoreMutateError::Denied => ToolError::Denied,
        StoreMutateError::BadInput(m) => ToolError::BadInput(m),
        StoreMutateError::Store(s) => ToolError::Extension(s.to_string()),
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing/invalid arg: {key}")))
}
