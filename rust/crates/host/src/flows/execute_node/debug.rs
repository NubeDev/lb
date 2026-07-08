//! The `debug` node — Node-RED's debug node (debug-node-scope). A terminal **observer**: reads the
//! wire envelope's `payload`, resolves its content `format`, and publishes a debug message onto the
//! per-flow debug subject as **motion** (fire-and-forget — no SurrealDB record, rule 3). Settles `Ok`
//! with the payload passed through, so `flow_node_state` records the envelope like any sink (Decision 5
//! last-value) but the debug *tail* itself is never state. Removing the node changes only what the
//! panel sees, never what the flow does (no downstream wires — `kind = sink`).
//!
//! Publish governor (Risk 1): a `rate_limit` (default 50 msgs/sec) caps real messages per node; a
//! breach publishes ONE `dropped: k` sentinel for the window instead of flooding the bus + every open
//! panel. Debug is best-effort motion, not a reliable log.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use lb_flows::{DEFAULT_COLLAPSE_BYTES, DEFAULT_RATE_LIMIT};
use serde_json::Value;

use crate::boot::Node;

use super::super::run_debug::{debug_message, dropped_event, publish_debug_event, resolve_format};
use super::super::run_store::NodeOutcome;

/// One sliding-window second.
const WINDOW: Duration = Duration::from_secs(1);

/// `debug` — publish the payload onto the flow's debug subject. Best-effort: a dropped publish is
/// non-fatal (the flow is unaffected; the panel simply misses it). The publish governor throttles a
/// hot source: at most `rate_limit` real messages per sliding second, plus one `dropped` sentinel.
#[allow(clippy::too_many_arguments)]
pub(super) async fn dispatch_debug(
    node: &Arc<Node>,
    ws: &str,
    flow_id: &str,
    run_id: &str,
    node_id: &str,
    config: &Value,
    inputs: &serde_json::Map<String, Value>,
    now: u64,
) -> NodeOutcome {
    let payload = inputs.get("payload").cloned().unwrap_or(Value::Null);
    let declared = config
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("auto");
    let format = resolve_format(declared, &payload);
    let label = config
        .get("label")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(node_id)
        .to_string();
    let collapse_bytes = config
        .get("collapse_bytes")
        .and_then(|v| v.as_u64())
        .unwrap_or(DEFAULT_COLLAPSE_BYTES);
    let rate_limit = config
        .get("rate_limit")
        .and_then(|v| v.as_u64())
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_RATE_LIMIT);

    // Publish governor (Risk 1). A per-(ws,flow,node) sliding window: at most `rate_limit` real
    // messages per second; over-budget messages collapse into one `dropped: k` sentinel at window
    // close. Per-key so two debug nodes don't share a budget; in-process (debug is best-effort and a
    // flow's owner node publishes locally — Decision 10). Same OnceLock-HashMap shape as `seed_lock`.
    let (publish_value, sentinel) = governor(ws, flow_id, node_id, now, rate_limit);
    if let Some(dropped) = sentinel {
        let ev = dropped_event(node_id, run_id, now, &label, dropped);
        publish_debug_event(&node.bus, ws, flow_id, &ev).await;
    }
    if publish_value {
        let ev = debug_message(
            node_id,
            run_id,
            now,
            format,
            &payload,
            &label,
            collapse_bytes,
        );
        publish_debug_event(&node.bus, ws, flow_id, &ev).await;
    }

    // Pass-through settle: the envelope is recorded like any sink (Decision 5), so the canvas badge
    // shows the value. The motion we just published is NOT the record — it's the projection.
    NodeOutcome::ok(serde_json::json!({ "payload": payload }))
}

/// One debug node's governor state — the head of a sliding 1s window, how many real messages it
/// admitted, and how many it suppressed (flushed as a `dropped` sentinel when the window closes).
#[derive(Default)]
struct GovernorState {
    window_start_ms: u64,
    count: u64,
    suppressed: u64,
}

/// Per-(ws,flow,node) governor slot. Minted lazily on first publish; lives for the node's lifetime.
type GovernorMap = HashMap<String, Arc<Mutex<GovernorState>>>;

fn governor_slot(ws: &str, flow_id: &str, node_id: &str) -> Arc<Mutex<GovernorState>> {
    static SLOTS: OnceLock<Mutex<GovernorMap>> = OnceLock::new();
    let map = SLOTS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = map.lock().expect("debug governor map poisoned");
    let composite = format!("{ws}\u{1}{flow_id}\u{1}{node_id}");
    guard
        .entry(composite)
        .or_insert_with(|| Arc::new(Mutex::new(GovernorState::default())))
        .clone()
}

/// Admit-or-suppress this message under the sliding-window cap. Returns
/// `(publish_this_message, flush_dropped_count)` — `flush_dropped_count` is `Some(k)` when the prior
/// window just closed with suppressed messages to report as a sentinel.
fn governor(
    ws: &str,
    flow_id: &str,
    node_id: &str,
    now: u64,
    rate_limit: u64,
) -> (bool, Option<u64>) {
    let slot = governor_slot(ws, flow_id, node_id);
    let mut g = slot.lock().expect("debug governor poisoned");
    let window_ms = WINDOW.as_millis() as u64;
    // Close the prior window if it has fully elapsed; flush a `dropped` sentinel if it accrued.
    let flush = if g.window_start_ms + window_ms <= now {
        let dropped = std::mem::replace(&mut g.suppressed, 0);
        g.window_start_ms = now;
        g.count = 0;
        (dropped > 0).then_some(dropped)
    } else if g.window_start_ms == 0 {
        // First-ever publish at this slot: start the window.
        g.window_start_ms = now;
        None
    } else {
        None
    };
    // Admit if the current window still has budget; else suppress (counted for the next flush).
    let publish = if g.count < rate_limit {
        g.count += 1;
        true
    } else {
        g.suppressed += 1;
        false
    };
    (publish, flush)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WS: &str = "ws-gov-test";
    const FLOW: &str = "f-gov-test";

    #[test]
    fn admits_up_to_rate_limit_in_one_window_then_suppresses() {
        let node = "admits";
        // limit 3 at t=0: three admitted, the rest suppressed (no flush yet — same window).
        for _ in 0..3 {
            assert_eq!(governor(WS, FLOW, node, 0, 3).0, true);
        }
        assert_eq!(governor(WS, FLOW, node, 0, 3).0, false);
        assert_eq!(governor(WS, FLOW, node, 0, 3).0, false);
    }

    #[test]
    fn flushes_a_dropped_sentinel_when_the_window_closes() {
        let node = "flushes";
        // window at t=0: admit 2 (limit), suppress 3.
        for _ in 0..2 {
            governor(WS, FLOW, node, 0, 2);
        }
        for _ in 0..3 {
            assert_eq!(governor(WS, FLOW, node, 0, 2).0, false);
        }
        // t=1100ms: new window — the 3 suppressed flush as a sentinel, then this msg admits.
        let (publish, flush) = governor(WS, FLOW, node, 1100, 2);
        assert!(publish);
        assert_eq!(flush, Some(3));
    }

    #[test]
    fn no_sentinel_when_a_window_had_no_suppressions() {
        let node = "clean";
        governor(WS, FLOW, node, 0, 5);
        let (_, flush) = governor(WS, FLOW, node, 1100, 5);
        assert_eq!(flush, None, "a clean window flushes nothing");
    }

    #[test]
    fn independent_budgets_per_node() {
        // node A exhausting its budget does NOT touch node B's.
        for _ in 0..3 {
            governor(WS, FLOW, "indep-a", 0, 3);
        }
        assert_eq!(governor(WS, FLOW, "indep-a", 0, 3).0, false, "A is full");
        assert_eq!(governor(WS, FLOW, "indep-b", 0, 3).0, true, "B is fresh");
    }
}
