//! `react_to_flow_approvals` — resume (or cancel) a flow run parked on an **approval gate** the moment
//! its inbox item resolves (rules-workflow-convergence scope, slice 4). The `approval` node parks the
//! run and writes a `needs:approval` item keyed `flow-approval:{run_id}:{node_id}` (the approval
//! node's `gate_item_id`); this scan reads the resolutions, parses that key, and drives the
//! parked run to its next state:
//!   - **`Approved`** → `flows.resume` the run (the gate node re-drives, reads the resolution, settles
//!     `Ok`, and the run continues).
//!   - **`Rejected`** → `flows.cancel` the run (terminal; the gate never releases downstream).
//!
//! Altitude — a durable scan, the twin of the generic `approval_reactor` and the cron reactor: the
//! store is the source of truth, so a restarted reactor re-reads `approved`/`rejected` and never
//! misses a resolution. Idempotent: `flows.resume` on an already-resumed run re-drives an empty
//! frontier (no-op); `flows.cancel` on a cancelled run is a no-op. Workspace-walled — the `approved`/
//! `rejected` scans and the resume/cancel all select `ws`'s namespace, so a ws-B tick can only touch
//! ws-B runs (mandatory isolation §7).
//!
//! It runs under the flow reactor's system principal (the node acting on its own durable runs), the
//! same authority the cron/interval reactors resume under — so the gate node's re-drive re-checks each
//! downstream node's own cap (no widening).

use std::sync::Arc;

use lb_auth::Principal;
use lb_inbox::{approved, rejected};

use crate::boot::Node;

use super::error::FlowsError;
use super::lifecycle::flows_cancel;
use super::run::flows_resume;

/// The gate item-id prefix — `flow-approval:{run_id}:{node_id}`. Only resolutions under this prefix
/// name a parked flow run; every other resolution (a plain inbox item, a rule's held-effect approval)
/// is ignored by this reactor.
const GATE_PREFIX: &str = "flow-approval:";

/// The outcome of one pass: how many parked runs were resumed (approved) / cancelled (rejected).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct FlowApprovalPass {
    pub resumed: usize,
    pub cancelled: usize,
}

/// Run one pass over workspace `ws` at logical time `now`: resume every run whose approval gate landed
/// `Approved`, cancel every run whose gate landed `Rejected`. Idempotent; returns the pass tally.
pub async fn react_to_flow_approvals(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    now: u64,
) -> Result<FlowApprovalPass, FlowsError> {
    let mut pass = FlowApprovalPass::default();

    for resolution in approved(&node.store, ws)
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))?
    {
        if let Some(run_id) = run_of(&resolution.item_id) {
            // Resume the parked run. A run that is already terminal / not suspended re-drives an empty
            // frontier — a harmless no-op — so the scan is safe to re-run.
            if flows_resume(node, principal, ws, run_id, now).await.is_ok() {
                pass.resumed += 1;
            }
        }
    }

    for resolution in rejected(&node.store, ws)
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))?
    {
        if let Some(run_id) = run_of(&resolution.item_id) {
            if flows_cancel(node, principal, ws, run_id).await.is_ok() {
                pass.cancelled += 1;
            }
        }
    }

    Ok(pass)
}

/// Parse the `run_id` out of a gate item id `flow-approval:{run_id}:{node_id}`. Returns `None` for any
/// item id that is not a flow-approval gate (so the reactor ignores non-flow resolutions). The
/// `node_id` is not needed to resume — the whole run re-drives, and the gate node re-reads its own
/// resolution — so we split off the prefix and the trailing `:{node_id}`.
fn run_of(item_id: &str) -> Option<&str> {
    let rest = item_id.strip_prefix(GATE_PREFIX)?;
    // `rest` is `{run_id}:{node_id}`; the run id is everything up to the LAST colon (a run id itself
    // may contain colons — e.g. a subflow run — and the node id never does).
    rest.rsplit_once(':').map(|(run_id, _node_id)| run_id)
}
