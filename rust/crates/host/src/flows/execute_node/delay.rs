//! The `delay` node — **durable delay + rate-limit** (Decision 16, resolving the Tier-C timer). A
//! `delay` holds a message on a durable timer and RESUMES after a restart — never an in-memory sleep
//! (spine Decision 9 forbids a parked async task; the durable record + the resume seam is the honest
//! park). When the timer has not elapsed the node returns [`Dispatched::Park`]: `execute_one` resets
//! it to `Enqueued` and suspends the run, and `flows.resume` with an advanced clock re-drives it. This
//! is the same suspend/resume seam the `subflow` park rides (Decision 11), applied to a timer instead
//! of a child run.
//!
//! State lives in the bounded-accumulator table (`flow_node_buffer:{ws}:{flow}:{node}`) as
//! `{ release_at?, last_release? }` — `delay` mode records the release instant on first arrival;
//! `rate` mode records the last release so it can space the next one.

use std::sync::Arc;

use lb_flows::Flow;
use lb_store::{read, write, Store};
use serde_json::{json, Value};

use crate::boot::Node;

use super::super::record::{node_scoped_id, FLOW_NODE_BUFFER_TABLE};
use super::super::run_store::NodeOutcome;
use super::Dispatched;

/// Dispatch a `delay` node: settle (release the payload) or park (suspend on the durable timer).
pub(super) async fn dispatch_delay(
    node: &Arc<Node>,
    ws: &str,
    flow: &Flow,
    node_id: &str,
    config: &Value,
    inputs: &serde_json::Map<String, Value>,
    now: u64,
) -> Dispatched {
    let payload = inputs.get("payload").cloned().unwrap_or(Value::Null);
    let mode = config
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("delay");
    let id = node_scoped_id(&flow.id, node_id);
    let record = match read(&node.store, ws, FLOW_NODE_BUFFER_TABLE, &id).await {
        Ok(v) => v,
        Err(e) => return Dispatched::Settled(NodeOutcome::Err(format!("delay: {e}"))),
    };

    let result = if mode == "rate" {
        rate_step(&node.store, ws, &id, config, &record, now).await
    } else {
        delay_step(&node.store, ws, &id, config, &record, now).await
    };
    match result {
        Ok(true) => Dispatched::Settled(NodeOutcome::ok(json!({ "payload": payload }))),
        Ok(false) => Dispatched::Park,
        Err(e) => Dispatched::Settled(NodeOutcome::Err(format!("delay: {e}"))),
    }
}

/// Fixed-delay: on first arrival stamp `release_at = now + ms`; release once `now >= release_at`
/// (clearing the record), else park. Returns `true` = release, `false` = park.
async fn delay_step(
    store: &Store,
    ws: &str,
    id: &str,
    config: &Value,
    record: &Option<Value>,
    now: u64,
) -> Result<bool, String> {
    let ms = config.get("ms").and_then(|v| v.as_u64()).unwrap_or(1000);
    let existing = record
        .as_ref()
        .and_then(|r| r.get("release_at"))
        .and_then(|v| v.as_u64());
    let release_at = existing.unwrap_or(now.saturating_add(ms));
    if now >= release_at {
        // Elapsed — clear the timer so a later re-fire of this node starts a fresh delay.
        write(store, ws, FLOW_NODE_BUFFER_TABLE, id, &json!({}))
            .await
            .map_err(|e| e.to_string())?;
        Ok(true)
    } else {
        if existing.is_none() {
            write(
                store,
                ws,
                FLOW_NODE_BUFFER_TABLE,
                id,
                &json!({ "release_at": release_at }),
            )
            .await
            .map_err(|e| e.to_string())?;
        }
        Ok(false)
    }
}

/// Rate-limit: release at most one message per `rate_ms`. Release (and stamp `last_release = now`)
/// when `now >= last_release + rate_ms` (or there is no prior release), else park until the spacing
/// elapses. Returns `true` = release, `false` = park.
async fn rate_step(
    store: &Store,
    ws: &str,
    id: &str,
    config: &Value,
    record: &Option<Value>,
    now: u64,
) -> Result<bool, String> {
    let rate_ms = config
        .get("rate_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(1000);
    let last = record
        .as_ref()
        .and_then(|r| r.get("last_release"))
        .and_then(|v| v.as_u64());
    let ready = match last {
        None => true,
        Some(t) => now >= t.saturating_add(rate_ms),
    };
    if ready {
        write(
            store,
            ws,
            FLOW_NODE_BUFFER_TABLE,
            id,
            &json!({ "last_release": now }),
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok(true)
    } else {
        Ok(false)
    }
}
