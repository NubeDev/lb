//! The MCP bridge for the store-admin verbs — host-native tools under the one MCP contract
//! (README §6.5). The UI, an agent, or an operator script reach `store.status`/`store.compact`
//! the SAME way they reach any tool: a qualified call with JSON in/out. Each verb authorizes
//! inside (defense-in-depth under the dispatcher's outer `mcp:` gate); denials are opaque.

use lb_auth::Principal;
use lb_mcp::ToolError;
use serde_json::Value;

use crate::boot::Node;

use super::compact::store_compact_enqueue;
use super::error::StoreAdminError;
use super::status::store_status_run;

/// Dispatch a store-admin MCP call. `store.status` takes no args; `store.compact` takes none
/// either (there is one store per node — nothing to address).
pub async fn call_store_admin_tool(
    node: &Node,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    _input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "store.status" => {
            let report = store_status_run(&node.store, principal, ws).map_err(to_tool)?;
            Ok(serde_json::to_value(report).unwrap_or(Value::Null))
        }
        "store.compact" => {
            let enq = store_compact_enqueue(&node.store, principal, ws, now_wall_ms())
                .await
                .map_err(to_tool)?;
            Ok(serde_json::to_value(enq).unwrap_or(Value::Null))
        }
        _ => Err(ToolError::NotFound),
    }
}

fn to_tool(e: StoreAdminError) -> ToolError {
    match e {
        StoreAdminError::Denied => ToolError::Denied,
        StoreAdminError::Store(s) => ToolError::Extension(s.to_string()),
    }
}

fn now_wall_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
