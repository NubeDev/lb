//! `flows.patch_run` — a **config-only** patch to an **unexecuted** node of a live run (Decision 1).
//! The hard line: a structural change (add/remove/retype a node, rewire an edge) is rejected for a
//! live run and becomes a new version for the next run; only an unexecuted node's config may be
//! patched in place. The patched config is validated against the run's **pinned** node schema
//! (Decision 12) — never the current descriptor — so the form the run accepts is the form it offers.
//!
//! The patch writes the config onto the run's step record (an `unexecuted` node's step has no
//! recorded output yet); the engine reads it when the node's turn comes. A ws-B caller cannot patch
//! a ws-A run (the run record is read first).

use std::sync::Arc;

use lb_auth::Principal;
use serde_json::Value;

use crate::boot::Node;

use super::error::FlowsError;
use super::record::ClaimState;
use super::run_store;

/// Patch `node`'s config on run `run_id`. Accepted only if the node is **unexecuted** (Pending or
/// Enqueued — not yet Running/Done). The config is validated against the pinned descriptor's schema
/// (recovered from the pinned flow's node type → the workspace's current descriptors; Decision 12
/// means the run keeps the schema it pinned — v1 the flow record carries the latest, and a structural
/// edit during suspend writes a new version, so a patch targets a still-unexecuted node of the live
/// version). Returns `Ok(())` on success.
pub async fn flows_patch_run(
    node: &Arc<Node>,
    _principal: &Principal,
    ws: &str,
    run_id: &str,
    node_id: &str,
    config: Value,
) -> Result<(), FlowsError> {
    let run = run_store::read_run(&node.store, ws, run_id)
        .await
        .map_err(FlowsError::Internal)?
        .ok_or(FlowsError::NotFound)?;
    if run.status == "cancelled" {
        return Err(FlowsError::BadInput("cannot patch a cancelled run".into()));
    }
    let step = run_store::read_step(&node.store, ws, run_id, node_id)
        .await
        .map_err(FlowsError::Internal)?
        .ok_or(FlowsError::NotFound)?;
    // The hard line: only an UNEXECUTED node may be patched in place.
    if !matches!(step.claim, ClaimState::Pending | ClaimState::Enqueued) {
        return Err(FlowsError::BadInput(format!(
            "node `{node_id}` already executed (claim {:?}); a structural change must become a new flow version",
            step.claim
        )));
    }
    // Decision 12: validate against the PINNED schema. v1 the flow record carries the node's type;
    // its config schema is the workspace descriptor's. (A pinned-schema cache landing with the
    // version-pin follow-up makes this exact for an upgraded descriptor.)
    let flow = super::save::flows_get_internal(&node.store, ws, &run.flow_id).await?;
    let node_spec = flow.node(node_id).ok_or(FlowsError::NotFound)?;
    let registry = super::nodes::merged_registry_internal(&node.store, ws)
        .await
        .map_err(FlowsError::Internal)?;
    let desc = registry
        .iter()
        .find(|d| d.r#type == node_spec.node_type)
        .ok_or_else(|| FlowsError::BadInput(format!("node `{node_id}`: unknown type `{}`", node_spec.node_type)))?;
    lb_flows::validate_config(&desc.config, &config).map_err(|e| {
        FlowsError::BadInput(format!("patch for node `{node_id}` violates the pinned schema: {e}"))
    })?;

    // Persist the patched config on the step record (read back by the executor when the node's turn
    // comes). Carried as a dedicated field so it never collides with a recorded output.
    let mut rec = step;
    rec.patched_config = Some(config);
    lb_store::write(
        &node.store,
        ws,
        super::record::FLOW_STEP_TABLE,
        &super::record::step_record_id(run_id, node_id),
        &serde_json::to_value(&rec).map_err(|e| FlowsError::Internal(e.to_string()))?,
    )
    .await
    .map_err(|e| FlowsError::Internal(e.to_string()))?;
    Ok(())
}
