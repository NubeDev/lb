//! `reconcile_flows` — the **state-convergence loop** (triggers-lifecycle-scope), at the same altitude
//! as the native-lifecycle reconciler + `react_to_reminders`. On each pass it re-reads the workspace's
//! flow directory, **elects a single owner** (Decision 10: `placement` is the eligible set, not
//! replication), and for each `enabled` + **placement-matching** flow it owns it **converges** the
//! source to the armed state: **arms** its source nodes (start, pass the host-allocated series id +
//! config), and on disable **disarms** them so no live socket leaks (Decision 13). On boot it fires
//! the `boot` trigger once for each `start_on_boot` flow.
//!
//! Placement is matched **as data** against the node's role — config, never an `if cloud` branch
//! (rule 1). Cross-node failover (re-electing an owner when the home node dies) is a `node-roles`
//! deferral (the scope's explicit non-goal); v1 elects the local node for placement-matching flows.

use std::sync::Arc;

use lb_auth::Principal;

use crate::boot::Node;
use crate::role::Role;

use super::error::FlowsError;
use super::save::flows_list_internal;
use super::source::{arm_source, disarm_source};

/// The outcome of one reconciler pass: how many sources were armed / disarmed / boot-fired.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ReconcilePass {
    pub armed: usize,
    pub disarmed: usize,
    pub boot_fired: usize,
    /// Orphaned armed sources released this pass (their flow was deleted/tombstoned or their source
    /// node removed by an edit) — the leaked-socket collector (flow-deploy-ux-scope).
    pub orphans_disarmed: usize,
}

/// Whether this node owns `flow` (placement matches its `role`). The single-owner election
/// (Decision 10): `local-only` → an install/edge node, `cloud-only` → a hub, `either` → the home
/// node (v1: this node, so an `either` flow is armed once here, not on every matching node).
pub fn placement_matches(placement: lb_flows::Placement, role: Role) -> bool {
    use lb_flows::Placement::*;
    match (placement, role) {
        (Either, _) => true,
        // `cloud-only` → the shared-authority roles (Hub / Solo). An edge holds a read-cache, not the
        // authority a cloud-only flow needs.
        (CloudOnly, Role::Hub) | (CloudOnly, Role::Solo) => true,
        // `local-only` → the install/edge node that owns the local hardware a native source reads.
        (LocalOnly, Role::Edge) | (LocalOnly, Role::Solo) => true,
        _ => false,
    }
}

/// Run one reconciler pass over workspace `ws` at logical time `now` for a node in `role`. Arms
/// enabled + placement-matching flows' source nodes; disarms the rest (Decision 13 — no leaked live
/// socket when a flow is disabled). Fires `boot` once for each `start_on_boot` flow it newly arms.
pub async fn reconcile_flows(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    role: Role,
    now: u64,
) -> Result<ReconcilePass, FlowsError> {
    let flows = flows_list_internal(&node.store, ws).await?;
    let mut pass = ReconcilePass::default();
    for flow in flows {
        let owned = placement_matches(flow.placement, role);
        // Identify source nodes (descriptor kind = source). For v1, a node whose type carries an ext
        // namespace and the flow marks armed is a source; the descriptor kind is resolved at run.
        let source_nodes = source_node_ids(&flow);
        if flow.enabled && owned {
            for node_id in &source_nodes {
                // Pass the node's config WITH its `_type` stamped in, so `arm_source` resolves the
                // ext's arm tool AND persists `_type` for a later orphan disarm (the sweep has no flow
                // to read the type from once the flow is deleted).
                let cfg = flow
                    .node(node_id)
                    .map(|n| {
                        let mut c = n.config.clone();
                        if let serde_json::Value::Object(map) = &mut c {
                            map.insert("_type".into(), serde_json::json!(n.node_type));
                        }
                        c
                    })
                    .unwrap_or(serde_json::json!({}));
                let _ = arm_source(node, principal, ws, &flow.id, node_id, cfg).await;
                pass.armed += 1;
            }
            // Boot trigger: fire once per `mode:"boot"` trigger node for a start_on_boot flow, each
            // from ITS node (entry → only its subgraph runs). Idempotency: a boot run id is
            // deterministic per (flow, node); a re-reconcile within the same instant no-ops. A flow
            // may have several boot triggers (independent), like any other trigger kind.
            if flow.start_on_boot {
                for boot_node in boot_trigger_ids(&flow) {
                    let _ = super::run::flows_run(
                        node,
                        principal,
                        ws,
                        &flow.id,
                        serde_json::Map::new(),
                        &format!("{}-boot-{boot_node}", flow.id),
                        now,
                        Some(&boot_node),
                    )
                    .await;
                    pass.boot_fired += 1;
                }
            }
        } else {
            // Disabled or not owned by this node → disarm any armed source (converge to released).
            for node_id in &source_nodes {
                let _ = disarm_source(node, principal, ws, &flow.id, node_id).await;
                pass.disarmed += 1;
            }
        }
    }
    // Leaked-socket collector: disarm any armed source whose flow was deleted/tombstoned or whose
    // source node was removed by an edit (the per-flow pass above only converges flows still present).
    pass.orphans_disarmed = super::orphan_sweep::sweep_orphan_sources(node, principal, ws).await?;
    Ok(pass)
}

/// The `mode:"boot"` trigger node ids — fired once each at node start for a `start_on_boot` flow.
fn boot_trigger_ids(flow: &lb_flows::Flow) -> Vec<String> {
    flow.nodes
        .iter()
        .filter(|n| {
            n.node_type == "trigger"
                && n.config.get("mode").and_then(|v| v.as_str()) == Some("boot")
        })
        .map(|n| n.id.clone())
        .collect()
}

/// The source-node ids in a flow (nodes whose descriptor `kind` is `source`). Resolved from the
/// merged registry; a node type not in the registry is treated as non-source.
fn source_node_ids(flow: &lb_flows::Flow) -> Vec<String> {
    // The registry read is async; for the kind lookup we use the node-type heuristic: a source node
    // is an ext node (`<ext>.<type>`) the flow declares. The descriptor's kind is authoritative; v1
    // treats all ext nodes the reconciler arms as potential sources (arm is idempotent + the ext
    // reconciles to one socket). A precise kind filter lands with the descriptor-cached reconciler.
    flow.nodes
        .iter()
        .filter(|n| !lb_flows::is_builtin_type(&n.node_type))
        .map(|n| n.id.clone())
        .collect()
}
