//! `sweep_orphan_sources` — the **leaked-socket collector** (flow-deploy-ux-scope). The per-flow
//! arm/disarm pass in `reconcile_flows` only converges sources of a flow **still in the list**; a
//! **deleted** (tombstoned) flow, or a source node **removed by an edit**, leaves its `arm_source`
//! marker (`flow_node_state:{flow}:{node}` = `{armed:true, series}`) with a live socket in the
//! extension that nothing ever disarms. This sweep is that missing convergence: it scans the armed
//! markers and disarms any whose owning flow/node no longer exists.
//!
//! Runs inside the same reconcile pass (workspace-scoped, owner-elected — no `if cloud`). Idempotent:
//! `disarm_source` clears the marker, so a second pass finds nothing. Self-healing after a crash
//! mid-delete (the marker persists; the next pass collects it). Decision 13 ("converge to released")
//! already lived in the reconciler for present flows — this extends it to orphans.

use std::collections::HashSet;
use std::sync::Arc;

use lb_auth::Principal;
use lb_store::Store;
use serde_json::Value;

use crate::boot::Node;

use super::error::FlowsError;
use super::record::FLOW_NODE_STATE_TABLE;
use super::save::flows_list_internal;
use super::scan_all::scan_all;
use super::source::disarm_source;

/// One armed-source marker: the `{flow, node}` it belongs to. An armed marker is a `flow_node_state`
/// row carrying `armed:true` AND a `series` (a value-only record carries neither — this distinguishes
/// an `arm_source` marker from a node's last-value record in the shared table).
struct ArmedMarker {
    flow_id: String,
    node_id: String,
}

/// Sweep the workspace's armed-source markers and disarm any orphan (its flow is deleted/tombstoned,
/// or its source node was removed from the flow). Returns how many were disarmed. Workspace-scoped;
/// never touches another ws's markers (the scan is ws-walled).
pub async fn sweep_orphan_sources(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
) -> Result<usize, FlowsError> {
    let markers = armed_markers(&node.store, ws).await?;
    if markers.is_empty() {
        return Ok(0);
    }
    // The live (node, flow) pairs — an armed marker is an orphan unless BOTH its flow is present
    // (non-tombstoned; `flows_list_internal` skips tombstones) AND that flow still declares the node.
    let flows = flows_list_internal(&node.store, ws).await?;
    let live: HashSet<(String, String)> = flows
        .iter()
        .flat_map(|f| f.nodes.iter().map(move |n| (f.id.clone(), n.id.clone())))
        .collect();

    let mut disarmed = 0;
    for m in markers {
        if live.contains(&(m.flow_id.clone(), m.node_id.clone())) {
            continue; // still owned — the arm/disarm pass converges it, not us.
        }
        // Orphan: the flow is gone/tombstoned or the node was removed. Release the socket + clear the
        // marker. A disarm error is logged, never fatal — the next pass retries (idempotent).
        if let Err(e) = disarm_source(node, principal, ws, &m.flow_id, &m.node_id).await {
            tracing::warn!(ws = %ws, flow = %m.flow_id, node = %m.node_id, error = %e, "orphan source disarm failed");
            continue;
        }
        disarmed += 1;
    }
    Ok(disarmed)
}

/// Scan `flow_node_state` for the armed-source markers. Ids are `{table}:{flow}:{node}`; the body is
/// under the store envelope's `data`. A row is a marker iff its body has `armed == true` AND a
/// `series` string (a last-value record has neither).
async fn armed_markers(store: &Store, ws: &str) -> Result<Vec<ArmedMarker>, FlowsError> {
    let rows = scan_all(store, ws, FLOW_NODE_STATE_TABLE)
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))?;
    let prefix = format!("{FLOW_NODE_STATE_TABLE}:");
    let mut out = Vec::new();
    for row in rows {
        let body = match &row.data {
            Value::Object(o) => o.get("data").cloned().unwrap_or(Value::Null),
            other => other.clone(),
        };
        let armed = body.get("armed").and_then(|v| v.as_bool()).unwrap_or(false);
        let has_series = body.get("series").and_then(|v| v.as_str()).is_some();
        if !armed || !has_series {
            continue;
        }
        // Recover `{flow}:{node}` from the row id. The flow id can itself contain no `:`-free
        // guarantee, but `arm_source` writes exactly `{flow}:{node}` and node ids are `:`-free (they
        // are canvas-generated slugs), so the LAST `:` splits node from flow.
        let Some(rest) = row.id.strip_prefix(&prefix) else {
            continue;
        };
        let Some((flow_id, node_id)) = rest.rsplit_once(':') else {
            continue;
        };
        out.push(ArmedMarker {
            flow_id: flow_id.to_string(),
            node_id: node_id.to_string(),
        });
    }
    Ok(out)
}
