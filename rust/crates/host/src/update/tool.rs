//! The `update.*` MCP dispatcher — one arm per verb, by **exact name**.
//!
//! Exact names, never an `update.` prefix: reserving a namespace against a hypothetical extension
//! whose id is `update` is the mistake `ext.list` already avoided (scope decision 6). The outer gate
//! in `tool_call.rs` resolves the collapsed cap through `gate_tool_for`; each verb re-runs its own
//! `authorize_tool` inside (defense in depth, exactly like every other host family).

use lb_auth::Principal;
use lb_mcp::ToolError;
use serde_json::Value;

use super::{apply, enrol, read};
use crate::boot::Node;

/// Every `update.*` verb, listed once. `tool_call.rs` splices this into `HOST_NATIVE_EXACT`, and the
/// static catalog asserts a row exists for each — so a verb cannot be dispatchable and invisible.
pub const UPDATE_VERBS: &[&str] = &[
    "update.status",
    "update.check",
    "update.apply",
    "update.rollback",
    "update.history",
    "update.credential.status",
    "update.credential.set",
    "update.credential.claim",
];

/// Dispatch one `update.*` verb. Anything outside [`UPDATE_VERBS`] is `NotFound` — the host reserves
/// only these eight names.
pub async fn call_update_tool(
    node: &Node,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "update.status" => read::status(node, principal, ws).await,
        "update.check" => read::check(node, principal, ws).await,
        "update.history" => read::history(node, principal, ws, input).await,
        "update.credential.status" => read::credential_status(node, principal, ws).await,
        "update.apply" => apply::apply(node, principal, ws, input).await,
        "update.rollback" => apply::rollback(node, principal, ws).await,
        "update.credential.set" => enrol::set(node, principal, ws, input).await,
        "update.credential.claim" => enrol::claim(node, principal, ws, input).await,
        _ => Err(ToolError::NotFound),
    }
}
