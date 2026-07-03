//! The MCP bridge for nav verbs — host-native tools under the one MCP contract (README §7). UI,
//! agents, and extensions reach `nav.*` the SAME way they reach any wasm tool: a qualified call with
//! JSON in/out. The MCP gate runs inside each verb FIRST (workspace-first, then `mcp:nav.<verb>:call`),
//! so a ws-B caller or one without the grant is refused before the verb runs (the mandatory deny +
//! isolation tests are real here). Host-native — the gateway routes `nav.*` here for the routed/agent
//! path; `nav.resolve` + `nav.pref.*` need the `&Node` (ext discovery), the CRUD verbs the store.
//!
//! `save`/`delete`/`share`/`set_default`/`pref.set` take their logical `now` from the args (the
//! caller's clock — determinism §3, never wall-clock in the verb), exactly as `dashboard.save` does.

use lb_auth::Principal;
use lb_mcp::ToolError;
use serde_json::{json, Value};

use super::model::{NavItem, Visibility};
use super::{
    nav_delete, nav_get, nav_list, nav_pref_get, nav_pref_set, nav_resolve, nav_save,
    nav_set_default, nav_share, NavError,
};
use crate::boot::Node;

/// Dispatch a `nav.<verb>` MCP call. `input` is the verb's JSON arguments; the return is the verb's
/// JSON result. Each verb authorizes first; denials are opaque (`ToolError::Denied`).
pub async fn call_nav_tool(
    node: &Node,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "nav.get" => {
            let n = nav_get(&node.store, principal, ws, str_arg(input, "id")?)
                .await
                .map_err(to_tool)?;
            Ok(serde_json::to_value(n).unwrap_or(Value::Null))
        }
        "nav.list" => {
            let rows = nav_list(&node.store, principal, ws)
                .await
                .map_err(to_tool)?;
            Ok(json!({ "navs": rows }))
        }
        "nav.save" => {
            let items: Vec<NavItem> = serde_json::from_value(arg(input, "items")?.clone())
                .map_err(|e| ToolError::BadInput(format!("items: {e}")))?;
            let n = nav_save(
                &node.store,
                principal,
                ws,
                str_arg(input, "id")?,
                str_arg(input, "title")?,
                items,
                u64_arg(input, "now")?,
            )
            .await
            .map_err(to_tool)?;
            Ok(serde_json::to_value(n).unwrap_or(Value::Null))
        }
        "nav.delete" => {
            nav_delete(
                &node.store,
                principal,
                ws,
                str_arg(input, "id")?,
                u64_arg(input, "now")?,
            )
            .await
            .map_err(to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "nav.share" => {
            let visibility = visibility_arg(input)?;
            let team = input.get("team").and_then(|v| v.as_str());
            let n = nav_share(
                &node.store,
                principal,
                ws,
                str_arg(input, "id")?,
                visibility,
                team,
                u64_arg(input, "now")?,
            )
            .await
            .map_err(to_tool)?;
            Ok(serde_json::to_value(n).unwrap_or(Value::Null))
        }
        "nav.set_default" => {
            nav_set_default(
                &node.store,
                principal,
                ws,
                str_arg(input, "id")?,
                u64_arg(input, "now")?,
            )
            .await
            .map_err(to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "nav.resolve" => {
            let resolved = nav_resolve(node, principal, ws).await.map_err(to_tool)?;
            Ok(serde_json::to_value(resolved).unwrap_or(Value::Null))
        }
        "nav.pref.get" => {
            let pref = nav_pref_get(&node.store, principal, ws)
                .await
                .map_err(to_tool)?;
            Ok(serde_json::to_value(pref).unwrap_or(Value::Null))
        }
        "nav.pref.set" => {
            let pref = nav_pref_set(
                &node.store,
                principal,
                ws,
                str_arg(input, "id")?,
                u64_arg(input, "now")?,
            )
            .await
            .map_err(to_tool)?;
            Ok(serde_json::to_value(pref).unwrap_or(Value::Null))
        }
        _ => Err(ToolError::NotFound),
    }
}

/// Map the nav gate's outcome onto the MCP tool error (denials opaque).
fn to_tool(e: NavError) -> ToolError {
    match e {
        NavError::Denied => ToolError::Denied,
        NavError::NotFound => ToolError::NotFound,
        NavError::BadInput(m) => ToolError::BadInput(m),
        NavError::Store(s) => ToolError::Extension(s.to_string()),
    }
}

fn arg<'a>(input: &'a Value, key: &str) -> Result<&'a Value, ToolError> {
    input
        .get(key)
        .ok_or_else(|| ToolError::BadInput(format!("missing arg: {key}")))
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    arg(input, key)?
        .as_str()
        .ok_or_else(|| ToolError::BadInput(format!("arg not a string: {key}")))
}

fn u64_arg(input: &Value, key: &str) -> Result<u64, ToolError> {
    arg(input, key)?
        .as_u64()
        .ok_or_else(|| ToolError::BadInput(format!("arg not a u64: {key}")))
}

/// Parse the `visibility` arg (`"private" | "team" | "workspace"`).
fn visibility_arg(input: &Value) -> Result<Visibility, ToolError> {
    match str_arg(input, "visibility")? {
        "private" => Ok(Visibility::Private),
        "team" => Ok(Visibility::Team),
        "workspace" => Ok(Visibility::Workspace),
        other => Err(ToolError::BadInput(format!("bad visibility: {other}"))),
    }
}
