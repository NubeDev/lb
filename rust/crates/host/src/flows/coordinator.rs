//! The coordinator — `start` + `drive` (flow-run-scope; the frontier driver ported from `rubix-cube`
//! via the retired chain engine — see docs `rules/rule-chains-scope.md` lineage). `start`
//! seeds the run + the frontier slots; `drive` runs the ready frontier (each `(node, fctx)` slot:
//! CAS-claim → resolve bindings → execute under `caller ∩ grant` → record outcome → release
//! dependents per join policy / apply failure policy), looping until the frontier exhausts, then
//! finalises. The durable per-slot records + the CAS claim make a restart resume the un-run slots
//! exactly once (Decision 8, now keyed on `(node, fctx)`).

use std::collections::HashMap;
use std::sync::Arc;

use lb_auth::Principal;
use lb_flows::{resolve_links, Flow, NodeDescriptor};
use serde_json::Value;

use crate::boot::Node;

use super::execute_node;
use super::record::ClaimState;
use super::run_store;

/// Seed the run (the coordinator record + the frontier slots). Idempotent on `run_id`.
///
/// flow-input-ports-scope Slice 3: the wireless `link-out`/`link-in` pair is resolved to ordinary
/// port-targeted edges HERE (on a transient copy), so the persisted `flow` record keeps the author's
/// wireless sugar (editor round-trip, no stale wires on delete) while the engine schedules ordinary
/// wires. The resolved graph is threaded through `drive` too (resolved identically — deterministic).
pub async fn start(
    node: &Arc<Node>,
    ws: &str,
    run_id: &str,
    flow: &Flow,
    params: &serde_json::Map<String, Value>,
    now: u64,
    entry: Option<&str>,
) -> Result<(), String> {
    let resolved = resolve_links(flow);
    run_store::create_run(&node.store, ws, run_id, &resolved, params, now, entry).await
}

/// The per-node input-port join policy, read once at drive start and pinned with the run's flow
/// version (flow-input-ports-scope: "fctx/policy are read once at run start and pinned with the
/// version"). Maps a node id → its descriptor (for `join_of(port)` + `primary_input()`). A node whose
/// descriptor is missing (an unknown type) defaults to `All` — the safe barrier (and the save lint
/// already rejected unknown types).
pub async fn policy_map(
    node: &Arc<Node>,
    ws: &str,
    flow: &Flow,
) -> Result<HashMap<String, NodeDescriptor>, String> {
    let registry = super::nodes::merged_registry_internal(&node.store, ws)
        .await
        .map_err(|e| e.to_string())?;
    let mut out = HashMap::new();
    for n in &flow.nodes {
        if let Some(d) = registry.iter().find(|d| d.r#type == n.node_type) {
            out.insert(n.id.clone(), d.clone());
        }
    }
    Ok(out)
}

/// Drive the run toward completion. Idempotent + resumable: re-driving reads the durable per-slot
/// state, claims only un-run ready slots (CAS), and finalises when every slot is terminal. Returns
/// when the frontier exhausts. A suspended run stops enqueuing the next frontier (the unexecuted
/// slots stay Pending/Enqueued); `flows.resume` re-drives.
///
/// **Mid-run control bites between frontier batches** (flow-runtime-control-scope): before each
/// batch the durable run status is re-read; a `cancelled`/`suspended` written by `flows.cancel`/
/// `flows.suspend` stops the drive — the remaining slots stay un-run (audit kept), which is what
/// makes Stop actually stop a backgrounded run. On any terminal exit a `run-finished` settle event
/// is published so a watcher retires its live controls.
pub async fn drive(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    run_id: &str,
    flow: &Flow,
    params: &serde_json::Map<String, Value>,
    now: u64,
) -> Result<(), String> {
    // flow-input-ports-scope Slice 3: resolve the wireless link pair on a transient copy (the same
    // resolution `start` used) so the engine schedules ordinary wires. The pinned `flow.version` is
    // untouched (the resolved copy shares it); `policy_map` + `read_run` see the resolved node set.
    let flow = resolve_links(flow);
    // Pin the per-node join policy + the run's subgraph for the whole drive (flow-input-ports-scope).
    let policies = policy_map(node, ws, &flow).await?;
    let subgraph = match run_store::read_run(&node.store, ws, run_id).await? {
        Some(r) => match r.entry_node.as_deref() {
            Some(e) => flow.reachable_from(e),
            None => flow.nodes.iter().map(|n| n.id.clone()).collect(),
        },
        None => flow.nodes.iter().map(|n| n.id.clone()).collect(),
    };
    loop {
        // Control check: a cancel/suspend landed since the last batch → stop driving this run.
        if let Some(status) = control_halt(node, ws, run_id).await? {
            publish_finished(node, ws, run_id, &status).await;
            return Ok(());
        }
        let ready = run_store::ready_slots(&node.store, ws, run_id).await?;
        if ready.is_empty() {
            break;
        }
        for (node_id, fctx) in ready {
            // Box the per-slot execution: a `subflow` node re-enters the run engine (drive → execute
            // → subflow → run_to_completion → drive), an async recursion the compiler can only prove
            // `Send` when the cycle is broken by a boxed future here.
            Box::pin(execute_node::execute_one(
                node, principal, ws, run_id, &flow, &node_id, &fctx, &policies, &subgraph, params,
                now,
            ))
            .await?;
        }
        if let Some(status) =
            run_store::finalize_if_complete(&node.store, ws, &flow, run_id).await?
        {
            publish_finished(node, ws, run_id, &status).await;
            break;
        }
    }
    Ok(())
}

/// Read the run's durable status; return `Some(status)` if it is a control-terminal the driver must
/// halt on (`cancelled`/`suspended`). A missing run (deleted mid-drive) also halts.
async fn control_halt(node: &Arc<Node>, ws: &str, run_id: &str) -> Result<Option<String>, String> {
    match run_store::read_run(&node.store, ws, run_id).await? {
        Some(run) if run.status == "cancelled" || run.status == "suspended" => Ok(Some(run.status)),
        Some(_) => Ok(None),
        None => Ok(Some("cancelled".into())),
    }
}

/// Publish the terminal `run-finished` settle event (best-effort motion).
async fn publish_finished(node: &Arc<Node>, ws: &str, run_id: &str, status: &str) {
    let event = super::watch::run_finished_event(status);
    super::watch::publish_flow_event(&node.bus, ws, run_id, &event).await;
}

/// (Legacy frontier helper, retained for any external caller.) The set of `Enqueued` node ids at the
/// empty firing context — the all-`all` frontier. The drive loop itself uses [`run_store::ready_slots`]
/// (the per-`(node, fctx)` view); this wraps it for callers that still think per-node.
#[allow(dead_code)]
async fn ready_frontier(
    store: &lb_store::Store,
    ws: &str,
    run_id: &str,
    _flow: &Flow,
) -> Result<Vec<String>, String> {
    let _ = ClaimState::Pending; // (keep the import used)
    Ok(run_store::ready_slots(store, ws, run_id)
        .await?
        .into_iter()
        .map(|(n, _)| n)
        .collect())
}
