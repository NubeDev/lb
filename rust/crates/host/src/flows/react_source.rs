//! `react_to_flow_sources` — the durable **series-event reactor** that fires a flow run per new
//! webhook hit (rules-workflow-convergence scope, slice 5). It is the inbound counterpart of the cron
//! reactor: same altitude/cadence (a stateless scan over a durable cursor, never a long-lived
//! subscription that could drop samples across a restart, rule 4).
//!
//! For each enabled flow's `webhook` source node it reads the core webhook's series
//! `webhook:{ws}:{webhook_id}` for samples with `seq > last_seq` (the node's durable cursor), fires
//! ONE run per new sample (entry = the source node → only its subgraph runs, Node-RED per-wire; the
//! sample's payload IS the firing envelope), then advances `last_seq` to the highest seq seen. So a
//! hit fires **exactly once** and a restart resumes from the durable cursor — no missed hit, no
//! double-fire. Coalescing is deliberate NOT done here (each hit is its own run) so no event is lost.
//!
//! The node owns NO endpoint/credential: the webhook service owns the hook + series (webhooks-scope);
//! this reactor only *reacts* to what lands there. Workspace-walled — the series read + the run fire +
//! the cursor write all select `ws`'s namespace, so a ws-B tick can only see/fire ws-B hits (§7). It
//! runs under the flow reactor's system principal (the node acting on its own durable flows), which
//! carries `mcp:series.read:call` + the flows run surface.

use std::sync::Arc;

use lb_auth::Principal;

use crate::boot::Node;

use super::error::FlowsError;
use super::record::FlowTriggerState;
use super::run;
use super::save::flows_list_internal;
use super::trigger_store::{read_cursor, webhook_triggers, write_cursor, WebhookTrigger};

/// The outcome of one source-reactor pass: how many runs it fired across all webhook sources.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SourceReactorPass {
    pub fired: usize,
}

/// Run one pass over workspace `ws` at logical time `now`: for every enabled flow's `webhook` source
/// node, fire a run per new hit on its hook's series and advance its cursor. Returns the pass tally.
pub async fn react_to_flow_sources(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    now: u64,
) -> Result<SourceReactorPass, FlowsError> {
    let mut pass = SourceReactorPass::default();
    let flows = flows_list_internal(&node.store, ws).await?;
    for flow in &flows {
        if !flow.enabled {
            continue;
        }
        for trig in webhook_triggers(flow) {
            fire_new_hits(node, principal, ws, &flow.id, &trig, now, &mut pass).await?;
        }
    }
    Ok(pass)
}

/// Read the hook's series for samples past the node's cursor, fire one run per new sample, then
/// advance the cursor to the highest seq fired. A run id derived from `(flow, node, seq)` makes each
/// firing idempotent (a re-scan before the cursor write finds the same job and no-ops in `flows.run`).
#[allow(clippy::too_many_arguments)]
async fn fire_new_hits(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    flow_id: &str,
    trig: &WebhookTrigger,
    now: u64,
    pass: &mut SourceReactorPass,
) -> Result<(), FlowsError> {
    let series = crate::webhook::WebhookRecord::series_for(ws, &trig.webhook_id);
    let cursor = read_cursor(&node.store, ws, flow_id, &trig.node_id)
        .await
        .map_err(FlowsError::Internal)?;
    let last_seq = cursor.as_ref().and_then(|c| c.last_seq);

    // Read committed samples strictly after the cursor. `from_seq` is inclusive in the ingest range
    // query, so ask for `last_seq + 1`; a first arm (no cursor) reads from the start.
    let from = last_seq.map(|s| s.saturating_add(1));
    let samples = crate::ingest::series_read_range(&node.store, principal, ws, &series, from, None)
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))?;
    if samples.is_empty() {
        return Ok(());
    }

    let mut highest = last_seq.unwrap_or(0);
    for sample in &samples {
        // Defensive: never re-fire a seq at/below the cursor (the range query is inclusive on `from`).
        if let Some(prev) = last_seq {
            if sample.seq <= prev {
                continue;
            }
        }
        let run_id = source_run_id(flow_id, &trig.node_id, sample.seq);
        // The sample's value IS the firing envelope payload; stash it directly as the source node's
        // param so the entry leg (`core::trigger`) emits it as `payload` (the same seam cron uses to
        // inject its scheduled instant — cron wraps a `{cron_ts}` object; a webhook emits the raw hit).
        let mut params = serde_json::Map::new();
        params.insert(trig.node_id.clone(), sample.payload.clone());
        // Fire FROM the source node (entry) so only its downstream subgraph runs.
        run::flows_run(
            node,
            principal,
            ws,
            flow_id,
            params,
            &run_id,
            now,
            Some(&trig.node_id),
        )
        .await?;
        highest = highest.max(sample.seq);
        pass.fired += 1;
    }

    // Advance the cursor to the highest seq fired (fire-once: a later pass reads only newer hits).
    persist_cursor(node, ws, flow_id, &trig.node_id, highest).await
}

/// A deterministic run id for a webhook firing: stable per (flow, node, sample seq) so a re-scan
/// before the cursor advanced re-drives the SAME run (a no-op), never a duplicate.
pub fn source_run_id(flow_id: &str, node_id: &str, seq: u64) -> String {
    format!("{flow_id}-hook-{node_id}-{seq}")
}

/// Persist the source node's cursor with `last_seq` advanced (preserving the record's other fields is
/// unnecessary — a webhook source uses only `last_seq`).
async fn persist_cursor(
    node: &Arc<Node>,
    ws: &str,
    flow_id: &str,
    node_id: &str,
    last_seq: u64,
) -> Result<(), FlowsError> {
    let state = FlowTriggerState {
        next_attempt_ts: 0,
        cron: None,
        period_secs: None,
        flop: None,
        last_seq: Some(last_seq),
    };
    write_cursor(&node.store, ws, flow_id, node_id, &state)
        .await
        .map_err(FlowsError::Internal)
}
