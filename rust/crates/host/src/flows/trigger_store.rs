//! Per-trigger-node reactive state (flow-multi-trigger-reactive-scope). This is the seam that lets a
//! flow hold **N independent triggers**: each cron/source node owns its own cursor in
//! `flow_trigger_state:{flow}:{node}`, instead of one flow-level `cron`/`next_attempt_ts`. The
//! reactor scans nodes, not flows, so two cron triggers on different schedules each fire on their own
//! clock and never collapse or reject each other (the single-schedule wall this tears out).
//!
//! State, not motion (rule 3): the cursor is durable in SurrealDB; the reactor is a stateless scan
//! over it (never a long-lived in-process timer). Workspace-walled — every read/write is ws-scoped.

use lb_flows::Flow;
use lb_reminders::is_valid;
use lb_store::Store;

use super::record::{node_scoped_id, FlowTriggerState, FLOW_TRIGGER_STATE_TABLE};

/// One cron trigger node: its id + its 5-field schedule (from the node's own `config.cron`).
pub struct CronTrigger {
    pub node_id: String,
    pub cron: String,
}

/// Every `mode:"cron"` trigger node in `flow` that carries a **valid** schedule. A flow may have any
/// number — they are independent. An invalid/empty spec is skipped (it arms nothing), never an error
/// that blocks the other triggers (one bad node must not freeze a flow's whole clock).
pub fn cron_triggers(flow: &Flow) -> Vec<CronTrigger> {
    flow.nodes
        .iter()
        .filter(|n| {
            n.node_type == "trigger"
                && n.config.get("mode").and_then(|v| v.as_str()) == Some("cron")
        })
        .filter_map(|n| {
            let cron = n.config.get("cron").and_then(|v| v.as_str())?.to_string();
            if cron.trim().is_empty() || !is_valid(&cron) {
                return None;
            }
            Some(CronTrigger {
                node_id: n.id.clone(),
                cron,
            })
        })
        .collect()
}

/// One flip-flop source node: its id + its hold interval (seconds). Default 10s when unset.
pub struct FlipFlopTrigger {
    pub node_id: String,
    pub period_secs: u64,
    pub start: bool,
}

/// Every `flipflop` source node in `flow`. A flow may have any number — each oscillates on its own
/// clock. An invalid (`< 1`) period is clamped to 1s rather than dropped (a source must always tick).
pub fn flipflop_triggers(flow: &Flow) -> Vec<FlipFlopTrigger> {
    flow.nodes
        .iter()
        .filter(|n| n.node_type == "flipflop")
        .map(|n| {
            let period_secs = n
                .config
                .get("period_secs")
                .and_then(|v| v.as_u64())
                .unwrap_or(10)
                .max(1);
            let start = n
                .config
                .get("start")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            FlipFlopTrigger {
                node_id: n.id.clone(),
                period_secs,
                start,
            }
        })
        .collect()
}

/// One `webhook` source node: its id + the webhook id its config names. The series it watches is the
/// core webhook's own `webhook:{ws}:{webhook_id}` — not a fresh `flow:…` series — because the webhook
/// service already owns the endpoint + credential + series (rules-workflow-convergence scope slice 5).
pub struct WebhookTrigger {
    pub node_id: String,
    pub webhook_id: String,
}

/// Every `webhook` source node in `flow` that names a webhook id. A flow may have any number — each
/// watches its own hook's series independently. A node with an empty `webhook_id` is skipped (it
/// watches nothing), never an error that blocks the flow's other sources.
pub fn webhook_triggers(flow: &Flow) -> Vec<WebhookTrigger> {
    flow.nodes
        .iter()
        .filter(|n| n.node_type == "webhook")
        .filter_map(|n| {
            let webhook_id = n
                .config
                .get("webhook_id")
                .and_then(|v| v.as_str())?
                .to_string();
            if webhook_id.trim().is_empty() {
                return None;
            }
            Some(WebhookTrigger {
                node_id: n.id.clone(),
                webhook_id,
            })
        })
        .collect()
}

/// Read a trigger node's durable cursor (`None` → never seen / no row yet).
pub async fn read_cursor(
    store: &Store,
    ws: &str,
    flow_id: &str,
    node_id: &str,
) -> Result<Option<FlowTriggerState>, String> {
    let raw = lb_store::read(
        store,
        ws,
        FLOW_TRIGGER_STATE_TABLE,
        &node_scoped_id(flow_id, node_id),
    )
    .await
    .map_err(|e| e.to_string())?;
    match raw {
        Some(v) => serde_json::from_value(v)
            .map(Some)
            .map_err(|e| e.to_string()),
        None => Ok(None),
    }
}

/// Write a trigger node's cursor (conflict-safe: the reactor and a concurrent save can both touch it).
pub async fn write_cursor(
    store: &Store,
    ws: &str,
    flow_id: &str,
    node_id: &str,
    state: &FlowTriggerState,
) -> Result<(), String> {
    let value = serde_json::to_value(state).map_err(|e| e.to_string())?;
    lb_store::write_locked(
        store,
        ws,
        FLOW_TRIGGER_STATE_TABLE,
        &node_scoped_id(flow_id, node_id),
        &value,
    )
    .await
    .map_err(|e| e.to_string())
}
