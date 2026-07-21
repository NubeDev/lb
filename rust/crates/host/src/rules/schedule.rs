//! The **schedule syncer** — compile a rule's `#[schedule(...)]` directive into a managed `cron → rule`
//! flow and reconcile it (scheduled-rules-scope, slice 2). Run as a side effect of `rules.save`, AFTER
//! the rule record (with its compiled `schedule` metadata) is persisted.
//!
//! ## The one architectural rule
//!
//! The directive is **authoring sugar compiled at save**, never a runtime construct. There is NO
//! rule-cron reactor — this syncer builds an ordinary enabled `cron` flow and the **existing**
//! `react_to_flows_cron` fires it. Nothing here scans directives on a firing tick. The managed flow is
//! *derived state*; the rule directive is the source of truth, re-asserted on every save.
//!
//! ## Scheduling = rule-write ∩ flow-write (no widening)
//!
//! The reconcile calls the **existing** `flows.save`/`flows.node.update`/`flows.delete` verbs under the
//! **same caller** that saved the rule. A caller with rule-write but not flow-write therefore gets a
//! clean deny at the flow write — we surface it as `pending: needs flow-write` (the rule + its schedule
//! metadata persist; the managed flow could not be built). No silent half-state, no widening: a rule
//! cannot gain flow-authoring authority its caller lacks.
//!
//! ## Managed flow shape
//!
//! `flow:{ws}:schedule:{rule_id}` (id = `schedule:{rule_id}`), `managed_by = "rule-schedule:{rule_id}"`,
//! `enabled`, `start_on_boot`, two nodes:
//!   - `cron` trigger  — `config = { mode: "cron", cron: <compiled> }`
//!   - `rule`  node     — `config = { rule: <rule_id> }`, `needs: [trigger]`

use std::sync::Arc;

use lb_auth::Principal;
use lb_flows::{Flow, Node};
use lb_reminders::next_after;
use serde_json::{json, Value};

use crate::boot::Node as HostNode;
use crate::flows::error::FlowsError;
use crate::flows::node_config::flows_node_update;
use crate::flows::save::{flows_delete, flows_get, flows_save};

use lb_rules::RuleSchedule;

/// How many upcoming firings the read surface previews (scheduled-rules-scope: "the next 5 runs").
const NEXT_RUNS: usize = 5;

/// The managed flow id for a rule schedule (`schedule:{rule_id}`; stored as `flow:{ws}:schedule:{id}`).
pub fn managed_flow_id(rule_id: &str) -> String {
    format!("schedule:{rule_id}")
}

/// The `managed_by` marker a rule-schedule flow carries.
pub fn managed_by_marker(rule_id: &str) -> String {
    format!("rule-schedule:{rule_id}")
}

/// Reconcile the managed `cron → rule` flow for `rule_id` against its compiled `schedule`:
///   - `Some(sched)` → create the managed flow (or update its trigger's `config.cron` if changed);
///   - `None`        → delete the managed flow (the directive was removed → run-on-demand).
///
/// Returns a JSON status block for the `rules.save` response so the caller learns what happened
/// (`managed`, `flow_id`, `cron`, `pending?`). A flow-write deny surfaces as `{pending: "needs
/// flow-write", ...}` — the schedule metadata already persisted, so the contract is explicit: the rule
/// is scheduled-in-intent but the managed flow was not built.
pub async fn sync_schedule(
    node: &Arc<HostNode>,
    principal: &Principal,
    ws: &str,
    rule_id: &str,
    schedule: Option<&RuleSchedule>,
) -> Value {
    match schedule {
        Some(sched) => match ensure_managed_flow(node, principal, ws, rule_id, sched).await {
            Ok(()) => json!({
                "managed": true,
                "flow_id": managed_flow_id(rule_id),
                "raw": sched.raw,
                "cron": sched.cron,
            }),
            Err(FlowsError::Denied) => json!({
                "managed": false,
                "pending": "needs flow-write",
                "raw": sched.raw,
                "cron": sched.cron,
            }),
            Err(e) => json!({ "managed": false, "error": e.to_string() }),
        },
        None => match delete_managed_flow(node, principal, ws, rule_id).await {
            Ok(()) => json!({ "managed": false }),
            Err(FlowsError::Denied) => {
                json!({ "managed": false, "pending": "needs flow-write to tear down" })
            }
            Err(e) => json!({ "managed": false, "error": e.to_string() }),
        },
    }
}

/// Create the managed flow if absent, else converge its trigger's cron to `sched.cron` (idempotent: an
/// unchanged directive re-save issues no write). Preserves the "one write door" split-grant contract by
/// going through the ordinary gated `flows.save`/`flows.node.update` verbs.
async fn ensure_managed_flow(
    node: &Arc<HostNode>,
    principal: &Principal,
    ws: &str,
    rule_id: &str,
    sched: &RuleSchedule,
) -> Result<(), FlowsError> {
    let flow_id = managed_flow_id(rule_id);
    match flows_get(&node.store, principal, ws, &flow_id).await {
        Ok(existing) => {
            // The trigger node id is stable (`trigger`); update only if the cron actually changed —
            // idempotent re-save is a no-op (no version bump, no write).
            let current = existing
                .node("trigger")
                .and_then(|n| n.config.get("cron"))
                .and_then(|v| v.as_str());
            if current == Some(sched.cron.as_str()) {
                return Ok(());
            }
            flows_node_update(
                &node.store,
                principal,
                ws,
                &flow_id,
                "trigger",
                trigger_config(&sched.cron),
            )
            .await
            .map(|_| ())
        }
        Err(FlowsError::NotFound) => {
            let mut flow = build_managed_flow(ws, rule_id, sched);
            flows_save(&node.store, principal, ws, &mut flow)
                .await
                .map(|_| ())
        }
        Err(e) => Err(e),
    }
}

/// Delete the managed flow (idempotent: an absent flow is a no-op). Called when the directive is gone.
async fn delete_managed_flow(
    node: &Arc<HostNode>,
    principal: &Principal,
    ws: &str,
    rule_id: &str,
) -> Result<(), FlowsError> {
    flows_delete(&node.store, principal, ws, &managed_flow_id(rule_id)).await
}

/// The desired managed flow record: `cron trigger → rule node`, enabled, start-on-boot, marked managed.
fn build_managed_flow(ws: &str, rule_id: &str, sched: &RuleSchedule) -> Flow {
    let mut trigger = Node::new("trigger", "trigger");
    trigger.config = trigger_config(&sched.cron);

    let mut rule = Node::new("rule", "rule");
    rule.config = json!({ "rule": rule_id });
    rule.needs = vec!["trigger".into()];

    let mut flow = Flow::new(ws, managed_flow_id(rule_id));
    flow.name = format!("schedule: {rule_id}");
    flow.nodes = vec![trigger, rule];
    flow.enabled = true;
    flow.start_on_boot = true;
    flow.managed_by = Some(managed_by_marker(rule_id));
    flow
}

/// The cron trigger node config.
fn trigger_config(cron: &str) -> Value {
    json!({ "mode": "cron", "cron": cron })
}

/// The read-side schedule block for `rules.get` (scheduled-rules-scope slice 3): `{ raw, cron,
/// next_runs, flow_id, managed, drift? }`. `next_runs` are the next [`NEXT_RUNS`] firings computed with
/// `croner` (`next_after`) from `now` — the SAME engine the reactor fires on, so the preview never
/// lies. `managed`/`drift` are read from the managed flow: `managed=false` ⇒ the flow is missing
/// (pending flow-write, or torn down); `drift=true` ⇒ the managed flow's cron was hand-edited away from
/// the directive (allow-and-flag; the next save re-asserts).
pub async fn schedule_block(
    node: &Arc<HostNode>,
    principal: &Principal,
    ws: &str,
    rule_id: &str,
    sched: &RuleSchedule,
    now: u64,
) -> Value {
    let flow_id = managed_flow_id(rule_id);
    let next_runs = next_runs(&sched.cron, now);

    // Read the managed flow (best-effort under the caller): present ⇒ managed; a diverged trigger cron
    // ⇒ drift. Absent/denied ⇒ not managed (pending or torn down) — no drift claim without a flow.
    let (managed, drift) = match flows_get(&node.store, principal, ws, &flow_id).await {
        Ok(flow) => {
            let flow_cron = flow
                .node("trigger")
                .and_then(|n| n.config.get("cron"))
                .and_then(|v| v.as_str());
            (true, flow_cron != Some(sched.cron.as_str()))
        }
        Err(_) => (false, false),
    };

    json!({
        "raw": sched.raw,
        "cron": sched.cron,
        "next_runs": next_runs,
        "flow_id": flow_id,
        "managed": managed,
        "drift": drift,
    })
}

/// The next [`NEXT_RUNS`] firing instants (logical seconds) strictly after `now`, via `croner`. An
/// invalid cron (should never reach here — validated at save) yields an empty list rather than panics.
fn next_runs(cron: &str, now: u64) -> Vec<u64> {
    let mut out = Vec::with_capacity(NEXT_RUNS);
    let mut cursor = now;
    for _ in 0..NEXT_RUNS {
        match next_after(cron, cursor) {
            Ok(next) if next > cursor => {
                out.push(next);
                cursor = next;
            }
            _ => break,
        }
    }
    out
}
