//! The MCP bridge for the DB-browser verbs — host-native tools under the one MCP contract (README
//! §6.5). UI and agents reach `store.tables`/`store.scan`/`store.graph` the same way they reach any
//! tool: a qualified call with JSON in/out. Each verb authorizes first (admin cap); a denial is
//! opaque (`ToolError::Denied`).
//!
//! Host-native (not a wasm extension), so it is NOT in the runtime `Registry`; the gateway/UI route
//! `store.*` here.

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

use super::{store_graph_view, store_scan_view, store_tables_view, DbViewError};

/// Dispatch a DB-browser MCP call. `input` is the verb's JSON arguments; the return is the verb's
/// JSON result. Each verb authorizes first; denials are opaque.
pub async fn call_dbview_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "store.tables" => {
            let tables = store_tables_view(store, principal, ws)
                .await
                .map_err(dbview_to_tool)?;
            Ok(json!({ "tables": tables }))
        }
        "store.scan" => {
            let table = str_arg(input, "table")?;
            let limit = input.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
            let after = input.get("cursor").and_then(|v| v.as_str());
            let page = store_scan_view(store, principal, ws, table, limit, after)
                .await
                .map_err(dbview_to_tool)?;
            Ok(json!({ "rows": page.rows, "next": page.next }))
        }
        "store.graph" => {
            let table = input.get("table").and_then(|v| v.as_str());
            let id = input.get("id").and_then(|v| v.as_str());
            let depth = input.get("depth").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
            let g = store_graph_view(store, principal, ws, table, id, depth)
                .await
                .map_err(dbview_to_tool)?;
            Ok(json!({ "nodes": g.nodes, "edges": g.edges }))
        }
        _ => Err(ToolError::NotFound),
    }
}

/// Map the DB-browser gate's outcome onto the MCP tool error. `Denied` stays `Denied` (no existence
/// signal); a store error surfaces as `Extension`.
fn dbview_to_tool(e: DbViewError) -> ToolError {
    match e {
        DbViewError::Denied => ToolError::Denied,
        DbViewError::Store(s) => ToolError::Extension(s.to_string()),
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing/invalid arg: {key}")))
}
