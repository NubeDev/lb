//! The `approval` gate node — **park the run on a human decision** (rules-workflow-convergence scope,
//! slice 4). It is the generic sibling of the retired coding-workflow approval gate: no `PrSpec`, no
//! job, no provider — just "hold this run until a reviewer approves". It parks on the SAME durable
//! suspend/resume machinery the `delay` node uses ([`Dispatched::Park`]), so a restart mid-wait
//! resumes cleanly.
//!
//! The state machine, keyed on the gate's inbox item `flow-approval:{run_id}:{node_id}`:
//!   - **no resolution yet** — ensure the `needs:approval` inbox item exists (idempotent upsert),
//!     then PARK (the run suspends; the flow-approval reactor resumes it when the item resolves).
//!   - **`Approved`** — settle `Ok`, passing the incoming envelope through (the run continues).
//!   - **`Rejected`** — settle `Err("rejected")` (the subtree is gated like any node failure).
//!   - **`Deferred`** — treated as still-pending: PARK again (a deferred item can later approve).
//!
//! The inbox write goes under the caller's own authority (`mcp:inbox.record:call`, `caller ∩ grant`) —
//! a flow whose gate node lacks the inbox cap is denied AT the node (no widening). The resolution READ
//! is a host-internal store read on the already-authorized run path (like the rule cage's reads).

use std::sync::Arc;

use lb_auth::Principal;
use lb_inbox::{resolution, Decision, Item};
use serde_json::{json, Value};

use crate::boot::Node;
use crate::tool_call::call_tool;

use super::super::run_store::NodeOutcome;
use super::Dispatched;

/// The default channel `needs:approval` gate items land on (the reviewers' inbox).
const DEFAULT_CHANNEL: &str = "approvals";

/// The gate item id for a run's approval node — the key the flow-approval reactor resolves against.
/// Stable per (run, node) so a resume re-reads the same resolution (no wall-clock/random).
pub fn gate_item_id(run_id: &str, node_id: &str) -> String {
    format!("flow-approval:{run_id}:{node_id}")
}

/// Dispatch an `approval` gate node: settle on a resolution, or park awaiting one. `config` carries
/// `{team, channel?}` — the routed team + the inbox channel (default `approvals`). The incoming
/// `payload` is passed through unchanged on approval (the gate holds, it does not transform).
#[allow(clippy::too_many_arguments)]
pub(super) async fn dispatch_approval(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    run_id: &str,
    node_id: &str,
    config: &Value,
    inputs: &serde_json::Map<String, Value>,
    now: u64,
) -> Dispatched {
    let item_id = gate_item_id(run_id, node_id);
    // Check the reviewer's decision (host-internal read on the authorized run path).
    match resolution(&node.store, ws, &item_id).await {
        Ok(Some(r)) if r.decision == Decision::Approved => {
            // Approved: continue, passing the held envelope through untouched.
            let payload = inputs.get("payload").cloned().unwrap_or(Value::Null);
            Dispatched::Settled(NodeOutcome::ok(json!({ "payload": payload })))
        }
        Ok(Some(r)) if r.decision == Decision::Rejected => {
            Dispatched::Settled(NodeOutcome::Err("rejected".into()))
        }
        // No resolution, or a Deferred one: ensure the gate item exists, then park.
        Ok(_) => match ensure_gate_item(node, principal, ws, config, &item_id, now).await {
            Ok(()) => Dispatched::Park,
            Err(e) => Dispatched::Settled(NodeOutcome::Err(e)),
        },
        Err(e) => Dispatched::Settled(NodeOutcome::Err(format!("approval: {e}"))),
    }
}

/// Write the `needs:approval` inbox item for this gate (idempotent upsert on the item id), under the
/// caller's own `mcp:inbox.record:call` authority. The body carries the routed team so a reviewer's
/// UI can filter it, exactly like any other inbox item.
async fn ensure_gate_item(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    config: &Value,
    item_id: &str,
    now: u64,
) -> Result<(), String> {
    let channel = config
        .get("channel")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_CHANNEL);
    let team = config.get("team").and_then(|v| v.as_str()).unwrap_or("");
    let body = format!("needs:approval route:team:{team}");
    // Route through the generic `inbox.record` verb (the item's author is forced to the principal's
    // `sub` inside the verb) so the gate write takes the same caps/audit path as any inbox write.
    let item = Item::new(item_id, channel, principal.sub(), body, now);
    let req = json!({
        "channel": channel,
        "id": item.id,
        "body": item.body,
        "ts": now,
    });
    match Box::pin(call_tool(
        node,
        principal,
        ws,
        "inbox.record",
        &req.to_string(),
    ))
    .await
    {
        Ok(_) => Ok(()),
        Err(e) => Err(super::tool_err_string(e)),
    }
}
