//! `flows.node.get` / `flows.node.update` — per-node config CRUD on a **saved** flow
//! (flow-runtime-control-scope). The gap they close: today the only way to change one node's config
//! is to re-`save` the whole `Flow` (a lost-update hazard + re-sends the entire graph). These verbs
//! read/replace exactly one node's `config` in place, validated against that node's descriptor schema
//! (the same per-node validator `flows.save` runs), bumping `flow.version` like any structural edit
//! (Decision 1 — a live run keeps its pinned version).
//!
//! Distinct from `flows.patch_run`: that targets an *unexecuted node of a live run* against the run's
//! PINNED schema; this targets the *saved flow's* current node against its CURRENT descriptor. Both
//! are config-only — topology/binding edits stay in `flows.save`.
//!
//! Gated at the bridge (`mcp:flows.node.get:call` / `mcp:flows.node.update:call`); here the
//! store-write/read surface re-runs the `flow` store gate (defense in depth) and the workspace wall
//! (read-first: a ws-B caller never sees/edits a ws-A flow).

use lb_auth::Principal;
use lb_flows::Flow;
use lb_store::{write, Store};
use serde_json::Value;

use super::error::FlowsError;
use super::nodes::merged_registry_internal;
use super::record::FLOW_TABLE;
use super::save::{authorize_store_read, authorize_store_write, flows_get_internal};

/// `flows.node.get {id, node}` — read one node's current config from the saved flow. Returns the
/// node's `{id, type, config}`. `NotFound` if the flow or the node is absent (workspace-walled).
pub async fn flows_node_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    flow_id: &str,
    node_id: &str,
) -> Result<Value, FlowsError> {
    authorize_store_read(principal, ws)?;
    let flow = flows_get_internal(store, ws, flow_id).await?;
    let n = flow.node(node_id).ok_or(FlowsError::NotFound)?;
    Ok(serde_json::json!({
        "id": n.id,
        "type": n.node_type,
        "config": n.config,
    }))
}

/// `flows.node.update {id, node, config}` — replace one node's config in place. Validates the new
/// config against the node's descriptor schema (rejects `BadInput` on a mismatch, leaving the record
/// unchanged), then bumps `flow.version` and persists. Returns `{id, node, version}`.
pub async fn flows_node_update(
    store: &Store,
    principal: &Principal,
    ws: &str,
    flow_id: &str,
    node_id: &str,
    config: Value,
) -> Result<Value, FlowsError> {
    authorize_store_write(principal, ws)?;
    let mut flow = flows_get_internal(store, ws, flow_id).await?;
    // Locate the node; an absent node is a NotFound (never a silent create — topology stays `save`).
    let idx = flow
        .nodes
        .iter()
        .position(|n| n.id == node_id)
        .ok_or(FlowsError::NotFound)?;
    let node_type = flow.nodes[idx].node_type.clone();

    // Validate the proposed config against the node's CURRENT descriptor schema (same validator as
    // `flows.save`). A rejection happens BEFORE any write — the record is untouched on a bad config.
    let registry = merged_registry_internal(store, ws)
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))?;
    let desc = registry
        .iter()
        .find(|d| d.r#type == node_type)
        .ok_or_else(|| {
            FlowsError::BadInput(format!(
                "node `{node_id}`: unknown type `{node_type}` (extension not installed in this workspace)"
            ))
        })?;
    lb_flows::validate_config(&desc.config, &config)
        .map_err(|e| FlowsError::BadInput(format!("node `{node_id}` ({node_type}): {e}")))?;

    // Apply + bump the version (Decision 1: a config change is a structural edit → a new version; a
    // live run keeps the version it pinned).
    flow.nodes[idx].config = config;
    flow.version = flow.version.saturating_add(1);
    persist(store, ws, &flow).await?;
    Ok(serde_json::json!({
        "id": flow.id,
        "node": node_id,
        "version": flow.version,
    }))
}

async fn persist(store: &Store, ws: &str, flow: &Flow) -> Result<(), FlowsError> {
    let value = serde_json::to_value(flow).map_err(|e| FlowsError::Internal(e.to_string()))?;
    write(store, ws, FLOW_TABLE, &flow.id, &value)
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))
}
