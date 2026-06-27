//! The MCP bridge for the read-only SQL verbs — host-native tools under the one MCP contract (README
//! §6.5). The widget bridge (a scripted view/control), the source picker, the visual SQL builder, and
//! any agent reach `store.query`/`store.schema` the SAME way they reach any tool: a qualified call
//! with JSON in/out. Each verb authorizes first (and `store.query` parse-allowlists); denials are
//! opaque (`ToolError::Denied`); a parse/reject reason surfaces as `BadInput` (author feedback).
//!
//! Host-native — not a wasm extension, so NOT in the runtime `Registry`; the bridge/gateway route
//! `store.query`/`store.schema` here.

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::Value;

use super::{store_query_run, store_schema_read, StoreQueryError};

/// Dispatch a read-only SQL MCP call. `input` is the verb's JSON arguments; the return is the verb's
/// JSON result. Each verb authorizes first; denials are opaque.
pub async fn call_store_query_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "store.query" => {
            let sql = str_arg(input, "sql")?;
            let vars = vars_arg(input)?;
            let result = store_query_run(store, principal, ws, sql, vars)
                .await
                .map_err(to_tool)?;
            Ok(serde_json::to_value(result).unwrap_or(Value::Null))
        }
        "store.schema" => {
            let schema = store_schema_read(store, principal, ws)
                .await
                .map_err(to_tool)?;
            Ok(serde_json::to_value(schema).unwrap_or(Value::Null))
        }
        _ => Err(ToolError::NotFound),
    }
}

/// Map the read-only SQL gate's outcome onto the MCP tool error. `Denied` stays opaque; a `Rejected`/
/// `Parse` reason surfaces as `BadInput` (it is author feedback for the editor, not an auth signal);
/// a store fault is `Extension`.
fn to_tool(e: StoreQueryError) -> ToolError {
    match e {
        StoreQueryError::Denied => ToolError::Denied,
        StoreQueryError::Rejected(m) => ToolError::BadInput(m),
        StoreQueryError::Parse(m) => ToolError::BadInput(format!("parse error: {m}")),
        StoreQueryError::Store(s) => ToolError::Extension(s.to_string()),
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing/invalid arg: {key}")))
}

/// Parse the optional `vars` arg (a JSON object) into `$`-bound query bindings. Absent → no bindings.
fn vars_arg(input: &Value) -> Result<Vec<(String, Value)>, ToolError> {
    match input.get("vars") {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Object(o)) => Ok(o.iter().map(|(k, v)| (k.clone(), v.clone())).collect()),
        Some(_) => Err(ToolError::BadInput("arg 'vars' must be an object".into())),
    }
}
