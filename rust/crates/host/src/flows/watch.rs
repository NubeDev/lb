//! `flows.watch {run_id}` + the per-node settle publisher (flow-runtime-control-scope). The headline
//! observability add: a watcher (the canvas, or any MCP/bus listener) sees a run's per-node settle
//! events live — `node-settled` deltas then a terminal `run-finished` — instead of only the final
//! `flows.runs.get` snapshot. This is **motion** (§3 rule 3): the durable per-node run records are
//! the record; this stream is a projection of them.
//!
//! Ordering is record-THEN-publish (mirrors `publish_run_event`): the driver persists a node's
//! outcome, then publishes the settle event, so a watcher never sees a node done the record doesn't
//! yet show. Fire-and-forget: a publish with no subscribers (or a transient bus error) is dropped —
//! a backgrounded run completes headless whether or not anyone is watching, and a late watcher
//! catches up from the snapshot.
//!
//! Workspace-walled two ways: `flows.watch` runs the `mcp:flows.watch:call` gate (opaque deny), and
//! the bus subject is prefixed `ws/{id}/` by `lb_bus` so a ws-B principal physically cannot
//! subscribe to a ws-A run's events (§7). Watch is read-only on the run — driving/cancelling needs
//! other caps.

use lb_auth::Principal;
use lb_bus::{publish, subscribe, Bus, Subscription};
use lb_mcp::authorize_tool;
use lb_store::Store;
use serde_json::{json, Value};

use super::error::FlowsError;
use super::runs::flows_runs_get;

/// The workspace-relative subject a run's settle stream rides on. `lb_bus` walls it under `ws/{id}/`
/// → `ws/{id}/flow/{run_id}/events`. `flow/` is a host-internal prefix (not a caller-nameable
/// `bus.*`/`ext/` subject), so it never collides with user subjects.
pub fn flow_run_subject(run_id: &str) -> String {
    format!("flow/{run_id}/events")
}

/// Publish one settle event for `run_id`. Best-effort: a serialization or bus failure is dropped (the
/// durable step records remain the source of truth; a late watcher catches up from the snapshot). The
/// payload is the JSON the SSE route forwards verbatim.
pub async fn publish_flow_event(bus: &Bus, ws: &str, run_id: &str, event: &Value) {
    let Ok(bytes) = serde_json::to_vec(event) else {
        return;
    };
    let _ = publish(bus, ws, &flow_run_subject(run_id), &bytes).await;
}

/// A `node-settled` event: one node reached terminal. Carries the same per-node shape the
/// `flows.runs.get` snapshot uses, so a watcher folds deltas into the same node colours.
pub fn node_settled_event(
    node_id: &str,
    outcome: &str,
    output: &Value,
    error: Option<&str>,
) -> Value {
    json!({
        "kind": "node-settled",
        "id": node_id,
        "outcome": outcome,
        "output": output,
        "error": error,
    })
}

/// A terminal `run-finished` event: the run reached a terminal status (`success`/`partialFailure`/
/// `failed`/`cancelled`/`suspended`). The watcher stops on this.
pub fn run_finished_event(status: &str) -> Value {
    json!({ "kind": "run-finished", "status": status })
}

/// What a watcher receives on attach: the catch-up `snapshot` (the current `flows.runs.get`
/// projection of the durable records) followed by the live `stream` of subsequent settle deltas. The
/// SSE route emits the snapshot first, then folds the stream — so a late join is seamless.
pub struct FlowWatch {
    /// The catch-up: the run snapshot as of attach (the same value `flows.runs.get` returns).
    pub snapshot: Value,
    /// The live delta feed for everything after the snapshot.
    pub stream: FlowEventSub,
}

/// A live subscription to one run's settle subject, decoded to JSON events. Mirrors `RunEventSub`.
pub struct FlowEventSub {
    inner: Subscription,
}

impl FlowEventSub {
    /// Await the next decoded settle event; skips an undecodable payload; `None` once closed.
    pub async fn recv(&self) -> Option<Value> {
        loop {
            let bytes = self.inner.recv().await?;
            match serde_json::from_slice::<Value>(&bytes) {
                Ok(event) => return Some(event),
                Err(_) => continue,
            }
        }
    }
}

/// `flows.watch {run_id}` — begin watching run `run_id` in workspace `ws` as `principal`. Gated
/// `mcp:flows.watch:call` (opaque deny). Subscribes to the run's settle subject, then reads the
/// current snapshot, so the catch-up + live feed compose. An absent/cross-workspace run still yields
/// a (possibly empty) snapshot and a live subscription — a watcher may attach before the first event.
pub async fn watch_flow_run(
    store: &Store,
    bus: &Bus,
    principal: &Principal,
    ws: &str,
    run_id: &str,
) -> Result<FlowWatch, FlowsError> {
    authorize_tool(principal, ws, "flows.watch").map_err(|_| FlowsError::Denied)?;
    // Subscribe BEFORE reading the snapshot so no delta slips through the gap between the snapshot
    // read and the subscription starting (a delta during the read is buffered; the events are
    // idempotent projections keyed by node id, so a one-event overlap is benign).
    let inner = subscribe(bus, ws, &flow_run_subject(run_id))
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))?;
    // The snapshot is the authorized read (its own `flow` store-read gate runs inside `flows_runs_get`).
    let snapshot = flows_runs_get(store, principal, ws, run_id)
        .await
        .unwrap_or_else(|_| json!({ "runId": run_id, "steps": [] }));
    Ok(FlowWatch {
        snapshot,
        stream: FlowEventSub { inner },
    })
}
