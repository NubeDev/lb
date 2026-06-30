//! Extension **source** nodes — the long-lived external feed shape (extension-nodes-scope, spine
//! Decision 2). A source can't be a request/response call: it's an external feed (an MQTT subscribe).
//! The host **arms** it when the flow enables (allocates the host-owned series
//! `flow:{ws}:{flow}:{node}`, validates the config, calls the ext's `arm` tool with the series id +
//! config) and **disarms** it when the flow disables (calls `disarm`, releasing the socket). The
//! event-trigger node then watches the series and fires a run per (coalesced) sample.
//!
//! The socket lives in the (supervised, native) extension — the flow instance stays **stateless**
//! (rule 4). The series name is **host-allocated** (Decision 2: ws-scoping + uniqueness are
//! host-owned, never extension-chosen). arming/disarming are ordinary declared `[[tools]]` the host
//! invokes — no new WIT world. The `ingest.write → series` bridge is the shipped MQTT-bridge path.
//!
//! This module owns the arm/disarm MECHANISM + the series allocation; the `reconcile_flows` loop
//! (triggers-lifecycle slice) CALLS it on enable/disable. Idempotent at the edges: a double-arm does
//! not open two sockets; a disarm-mid-arm still releases.

use std::sync::Arc;

use lb_auth::Principal;
use lb_mcp::ToolError;
use serde_json::{json, Value};

use crate::boot::Node;

use super::nodes::merged_registry_internal;
use super::record::FLOW_NODE_STATE_TABLE;

/// The host-allocated series id for a source node (Decision 2): `flow:{ws}:{flow}:{node}`. Host-owned
/// so ws-scoping + uniqueness never depend on the extension.
pub fn source_series(ws: &str, flow_id: &str, node_id: &str) -> String {
    format!("flow:{ws}:{flow_id}:{node_id}")
}

/// Arm a source node: allocate the series + call the extension's `arm` tool with the series id + the
/// validated node config. The extension (a native sidecar for an MQTT source) opens its socket and
/// bridges incoming events onto the series via `ingest.write`. `arm`/`disarm` are ordinary declared
/// `[[tools]]` — dispatched under the flow owner's principal (`caller ∩ install-grant`).
///
/// Returns the allocated series id (the event-trigger watches it). Idempotent: re-arming a node whose
/// series is already armed re-calls `arm` (the extension reconciles to one socket).
pub async fn arm_source(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    flow_id: &str,
    node_id: &str,
    config: Value,
) -> Result<String, ToolError> {
    let series = source_series(ws, flow_id, node_id);
    // Resolve the bound `<ext>.arm` tool from the descriptor (the node type's ext).
    let node_type = config.get("_type").and_then(|v| v.as_str()).unwrap_or("");
    let arm_tool = resolve_arm_tool(&node.store, ws, node_type).await;
    // Record the armed series as the node's last-value state (stateless flow; the socket is motion
    // owned by the extension). A re-arm overwrites (idempotent).
    lb_store::write(
        &node.store,
        ws,
        FLOW_NODE_STATE_TABLE,
        &format!("{flow_id}:{node_id}"),
        &json!({ "armed": true, "series": series }),
    )
    .await
    .map_err(|e| ToolError::Extension(e.to_string()))?;
    if let Some(tool) = arm_tool {
        // Dispatch the ext's `arm` tool under the owner's principal (caller ∩ install-grant). A denied
        // arm is logged + returned honestly — the source is not armed.
        let req = json!({ "series": series, "config": config }).to_string();
        let _ = crate::tool_call::call_tool(node, principal, ws, &tool, &req).await;
    }
    Ok(series)
}

/// Disarm a source node: call the extension's `disarm` tool (releases the socket — no leaked live
/// socket when a flow is disabled, Decision 13) and clear the armed marker. Idempotent.
pub async fn disarm_source(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    flow_id: &str,
    node_id: &str,
) -> Result<(), ToolError> {
    let node_type = lb_store::read(
        &node.store,
        ws,
        FLOW_NODE_STATE_TABLE,
        &format!("{flow_id}:{node_id}"),
    )
    .await
    .map_err(|e| ToolError::Extension(e.to_string()))?
    .and_then(|v| v.get("_type").and_then(|t| t.as_str()).map(String::from))
    .unwrap_or_default();
    if let Some(tool) = resolve_disarm_tool(&node.store, ws, &node_type).await {
        let req = json!({ "series": source_series(ws, flow_id, node_id) }).to_string();
        let _ = crate::tool_call::call_tool(node, principal, ws, &tool, &req).await;
    }
    lb_store::write(
        &node.store,
        ws,
        FLOW_NODE_STATE_TABLE,
        &format!("{flow_id}:{node_id}"),
        &json!({ "armed": false }),
    )
    .await
    .map_err(|e| ToolError::Extension(e.to_string()))?;
    Ok(())
}

/// Resolve `<ext>.arm` for a source node type (the canonical arm tool name; an extension may also
/// declare its own — this resolves `<ext_id>.arm` from the descriptor's ext namespace).
async fn resolve_arm_tool(store: &lb_store::Store, ws: &str, node_type: &str) -> Option<String> {
    let ext_id = node_type.split_once('.').map(|(e, _)| e)?;
    let _ = merged_registry_internal(store, ws).await; // (presence check: the ext is installed)
    Some(format!("{ext_id}.arm"))
}

async fn resolve_disarm_tool(store: &lb_store::Store, ws: &str, node_type: &str) -> Option<String> {
    let ext_id = node_type.split_once('.').map(|(e, _)| e)?;
    let _ = merged_registry_internal(store, ws).await;
    Some(format!("{ext_id}.disarm"))
}
