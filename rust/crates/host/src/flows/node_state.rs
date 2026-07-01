//! `flows.node_state {id}` — the **persistent runtime view** (flow-persistent-runtime-scope). Returns
//! every node's CURRENT last-value from `flow_node_state:{flow}:{node}` (Decision 5: one upserted
//! record per node, updated in place each scan), plus the flow's armed fields. This is the
//! steady-state the canvas paints — the Node-RED "each wire shows its current value", independent of
//! any single run. State, not motion (rule 3): readable any time, whether or not a run is in flight.
//!
//! Distinct from `flows.runs.get` (one finite run's per-node progress) and `flows.node.get` (a node's
//! saved CONFIG). This is the live VALUE. Gated `flows.node_state:call`; workspace-walled (read-first).

use lb_auth::Principal;
use lb_store::{scan, Store, MAX_SCAN_LIMIT};
use serde_json::{json, Value};

use super::error::FlowsError;
use super::record::{FLOW_INPUT_TABLE, FLOW_NODE_STATE_TABLE};
use super::save::{authorize_store_read, flows_get_internal};

/// `flows.node_state {id}` — every node's current persistent value for flow `flow_id`, plus the
/// flow's armed fields (so one read drives the whole canvas steady-state view).
pub async fn flows_node_state(
    store: &Store,
    principal: &Principal,
    ws: &str,
    flow_id: &str,
) -> Result<Value, FlowsError> {
    authorize_store_read(principal, ws)?;
    // Read the flow (ws-walled, NotFound→Denied) so we can return its armed fields AND know which node
    // ids belong to it (the canvas paints by node id).
    let flow = flows_get_internal(store, ws, flow_id).await?;

    // Scan the per-node last-value records; ids are `{flow}:{node}`. We filter to THIS flow's prefix so
    // one flow's view never bleeds another's (the records share one table per ws).
    let page = scan(store, ws, FLOW_NODE_STATE_TABLE, MAX_SCAN_LIMIT, None)
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))?;
    // `lb_store::scan` returns `Row.id` as the full `{table}:{flow}:{node}` string, so strip the table
    // segment AND the flow prefix to recover the node id (records for OTHER flows in the same table
    // don't match this prefix and are skipped — one flow's view never bleeds another's).
    let prefix = format!("{FLOW_NODE_STATE_TABLE}:{flow_id}:");

    let mut nodes = Vec::new();
    for row in page.rows {
        let Some(node_id) = row.id.strip_prefix(&prefix) else {
            continue;
        };
        // The stored body is under `data` (the store envelope); the rev is the optimistic token (it
        // bumps each in-place update — the canvas can see a value changed without diffing the value).
        let (value, rev) = match &row.data {
            Value::Object(o) => (
                o.get("data").cloned().unwrap_or(Value::Null),
                o.get("rev").cloned().unwrap_or(Value::Null),
            ),
            other => (other.clone(), Value::Null),
        };
        nodes.push(json!({ "node": node_id, "value": value, "rev": rev }));
    }

    // A node that has never produced a value has no `flow_node_state` row yet — include it with a null
    // value so the canvas can render every node, not only the ones that have run.
    for n in &flow.nodes {
        if !nodes.iter().any(|e| e["node"] == json!(n.id)) {
            nodes.push(json!({ "node": n.id, "value": Value::Null, "rev": Value::Null }));
        }
    }

    // Fold each node's RETAINED INPUT (`flow_input`) into its entry, so a control seeds its current
    // state from its OWN input (not its output) — one read drives both the canvas and the dashboard
    // (flow-dashboard-binding-ux-scope, Decision: read-back via node_state, no new verb). `input` is
    // the node-level retained `payload`; `inputs` is the per-port map. Records share one table per ws;
    // ids are `{flow}:{node}` (node-level) or `{flow}:{node}:{port}` (per-port).
    let input_page = scan(store, ws, FLOW_INPUT_TABLE, MAX_SCAN_LIMIT, None)
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))?;
    for row in input_page.rows {
        let body = match &row.data {
            Value::Object(o) => o.get("data").cloned().unwrap_or(Value::Null),
            other => other.clone(),
        };
        // The body carries the authoritative `flow`/`node`/`port?`/`value` (written by flows.inject);
        // filter to THIS flow so one flow's inputs never bleed another's.
        if body.get("flow").and_then(|v| v.as_str()) != Some(flow_id) {
            continue;
        }
        let Some(node_id) = body.get("node").and_then(|v| v.as_str()) else {
            continue;
        };
        let value = body.get("value").cloned().unwrap_or(Value::Null);
        let Some(entry) = nodes.iter_mut().find(|e| e["node"] == json!(node_id)) else {
            continue;
        };
        match body.get("port").and_then(|v| v.as_str()) {
            Some(port) => {
                if !entry["inputs"].is_object() {
                    entry["inputs"] = json!({});
                }
                entry["inputs"][port] = value;
            }
            None => entry["input"] = value,
        }
    }

    // Per-trigger armed state: each cron trigger node owns its own schedule + cursor (N independent
    // triggers). Attach `{cron, nextAttemptTs, armed}` to that node's entry, and compute a flow-level
    // summary (the SOONEST upcoming fire across all triggers) for the existing armed banner.
    let mut soonest: Option<u64> = None;
    let mut summary_cron: Option<String> = None;
    for trig in super::trigger_store::cron_triggers(&flow) {
        let cursor = super::trigger_store::read_cursor(store, ws, flow_id, &trig.node_id)
            .await
            .map_err(FlowsError::Internal)?;
        let next_ts = cursor.as_ref().map(|c| c.next_attempt_ts).unwrap_or(0);
        if flow.enabled && next_ts > 0 && soonest.map(|s| next_ts < s).unwrap_or(true) {
            soonest = Some(next_ts);
            summary_cron = Some(trig.cron.clone());
        }
        if let Some(entry) = nodes.iter_mut().find(|e| e["node"] == json!(trig.node_id)) {
            entry["cron"] = json!(trig.cron);
            entry["nextAttemptTs"] = json!(next_ts);
            entry["armed"] = json!(flow.enabled);
        }
    }

    Ok(json!({
        "flowId": flow_id,
        "enabled": flow.enabled,
        // Flow-level summary = the soonest-firing trigger (back-compat for the armed banner). The
        // authoritative per-trigger schedules live on each trigger node's entry above.
        "cron": summary_cron,
        "nextAttemptTs": soonest.unwrap_or(0),
        "nodes": nodes,
    }))
}
