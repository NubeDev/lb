//! The extension-node legs of the platform pack (ext-store-nodes scope): `ext-list` / `ext-call`.
//! Both dispatch through [`call_tool_node`] under the **caller's** principal, so the existing gates
//! apply per node execution — `mcp:ext.list:call` for the list, `mcp:<ext>.<tool>:call` (plus the
//! `caller ∩ install-grant` narrowing) for the call. The `ext`/`tool` config values are **opaque
//! strings** the author picked in the editor (rule 10 — core names no extension; swapping one
//! changes zero code here).

use std::sync::Arc;

use lb_auth::Principal;
use serde_json::{json, Value};

use crate::boot::Node;

use super::super::run_store::NodeOutcome;
use super::{call_tool_node, merge_tool_args};

/// `ext-list`: dispatch `ext.list` and emit the workspace's install rows as the `payload` array.
/// The verb takes no filter args, so `running_only` is applied host-side over the returned rows
/// (each carries the live `running` flag the verb joined in).
pub(super) async fn ext_list(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    config: &Value,
) -> NodeOutcome {
    let running_only = config
        .get("running_only")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    match call_tool_node(node, principal, ws, "ext.list", &json!({})).await {
        NodeOutcome::Ok { emitted, .. } => {
            let mut rows = emitted
                .get("extensions")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            if running_only {
                rows.retain(|r| r.get("running").and_then(|v| v.as_bool()).unwrap_or(false));
            }
            NodeOutcome::ok(json!({ "payload": rows }))
        }
        other => other,
    }
}

/// `ext-call`: dispatch the picked `<ext>.<tool>` with `config.args` merged with an object
/// `payload` (the `tool` node's exact rule, via the shared [`merge_tool_args`]); the tool's result
/// becomes the emitted `payload`. Both parts of the qualified verb come from config as opaque
/// strings — required, since without them there is nothing to dispatch.
pub(super) async fn ext_call(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    config: &Value,
    inputs: &serde_json::Map<String, Value>,
) -> NodeOutcome {
    let ext = config.get("ext").and_then(|v| v.as_str()).unwrap_or("");
    let tool = config.get("tool").and_then(|v| v.as_str()).unwrap_or("");
    if ext.is_empty() {
        return NodeOutcome::Err("ext-call node missing config.ext".into());
    }
    if tool.is_empty() {
        return NodeOutcome::Err("ext-call node missing config.tool".into());
    }
    let verb = format!("{ext}.{tool}");
    let args = merge_tool_args(config, inputs);
    match call_tool_node(node, principal, ws, &verb, &args).await {
        NodeOutcome::Ok { emitted, .. } => NodeOutcome::ok(json!({ "payload": emitted })),
        other => other,
    }
}
