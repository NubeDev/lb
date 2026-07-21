//! Tier-B (stateful) + Tier-C (engine-extending) integration tests for the data/JSON node pack
//! (data-nodes scope) — the honest hard ones, against the **real** store (`mem://`), real caps, real
//! jobs (CLAUDE §9). Proves:
//!
//! - **Workspace-isolation (mandatory):** a `batch` accumulator filled in ws1 is invisible to the
//!   same flow id in ws2 (the `{ws}:` prefix is the hard wall).
//! - **Tier B two-firing + cross-run persistence:** `filter` suppresses an unchanged second firing;
//!   `batch` releases at its count boundary; `unique`-stream dedupes — state survives across runs
//!   (the store is the durable seam — a restart re-opens the same store).
//! - **Tier C:** `switch` fires only the matched port's dependents (the other branch does NOT run);
//!   `split`→`join` round-trips a 3-element array with `parts`; `split`→`map`→`join` transforms each
//!   element; `delay` parks and RESUMES across a simulated restart (durable park, not a sleep).

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_flows::{FailurePolicy, Flow, Node, Placement};
use lb_host::{call_tool, Node as HostNode};
use serde_json::{json, Map, Value};

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:test".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).unwrap()
}

const FULL: &[&str] = &[
    "mcp:flows.save:call",
    "mcp:flows.run:call",
    "mcp:flows.resume:call",
    "mcp:flows.runs.get:call",
    "store:flow:write",
    "store:flow:read",
];

fn mknode(id: &str, ty: &str, needs: &[&str], with: Map<String, Value>, config: Value) -> Node {
    Node {
        id: id.into(),
        node_type: ty.into(),
        needs: needs.iter().map(|s| s.to_string()).collect(),
        with,
        config,
        inputs: Vec::new(),
        position: None,
    }
}

fn with_payload(p: Value) -> Map<String, Value> {
    Map::from_iter([("payload".into(), p)])
}

fn flow(ws: &str, id: &str, nodes: Vec<Node>) -> Flow {
    Flow {
        workspace: ws.into(),
        id: id.into(),
        name: id.into(),
        version: 0,
        params: Default::default(),
        nodes,
        failure_policy: FailurePolicy::Continue,
        deleted: false,
        enabled: true,
        start_on_boot: false,
        placement: Placement::Either,
        concurrency: Default::default(),
        cron: None,
        next_attempt_ts: 0,
        managed_by: None,
    }
}

async fn save(node: &Arc<HostNode>, p: &Principal, ws: &str, flow: &Flow) {
    let body = serde_json::to_value(flow).unwrap().to_string();
    call_tool(node, p, ws, "flows.save", &body).await.unwrap();
}

async fn runs_get(node: &Arc<HostNode>, p: &Principal, ws: &str, run_id: &str) -> Value {
    let req = json!({ "run_id": run_id }).to_string();
    let out = call_tool(node, p, ws, "flows.runs.get", &req)
        .await
        .unwrap();
    serde_json::from_str(&out).unwrap()
}

async fn await_status(node: &Arc<HostNode>, p: &Principal, ws: &str, run_id: &str) -> Value {
    for _ in 0..600 {
        let snap = runs_get(node, p, ws, run_id).await;
        let s = snap["status"].as_str().unwrap_or("");
        if matches!(
            s,
            "success" | "partialFailure" | "failed" | "cancelled" | "suspended"
        ) {
            return snap;
        }
        tokio::time::sleep(std::time::Duration::from_millis(3)).await;
    }
    panic!("run {run_id} did not settle");
}

/// Fire flow `id` in `ws` with run id `run_id` at time `ts`, awaiting a terminal/suspended status.
async fn fire(
    node: &Arc<HostNode>,
    p: &Principal,
    ws: &str,
    id: &str,
    run_id: &str,
    ts: u64,
) -> Value {
    let req = json!({ "id": id, "run_id": run_id, "ts": ts }).to_string();
    call_tool(node, p, ws, "flows.run", &req).await.unwrap();
    await_status(node, p, ws, run_id).await
}

fn outcome(snap: &Value, id: &str) -> String {
    snap["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == id)
        .map(|s| s["outcome"].as_str().unwrap_or("").to_string())
        .unwrap_or_default()
}

fn output(snap: &Value, id: &str) -> Value {
    snap["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == id)
        .map(|s| s["output"].clone())
        .unwrap_or(Value::Null)
}

// ---------------- Tier B: filter (RBE) ----------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn filter_suppresses_the_unchanged_second_firing() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let f = flow(
        "ws",
        "flt",
        vec![mknode(
            "f",
            "filter",
            &[],
            with_payload(json!(21)),
            json!({"mode": "changed"}),
        )],
    );
    save(&node, &p, "ws", &f).await;
    // First firing passes (nothing to compare).
    let s1 = fire(&node, &p, "ws", "flt", "flt-1", 1).await;
    assert_eq!(outcome(&s1, "f"), "ok", "first firing passes");
    // Second firing, same value → suppressed (skipped).
    let s2 = fire(&node, &p, "ws", "flt", "flt-2", 2).await;
    assert_eq!(
        outcome(&s2, "f"),
        "skipped",
        "unchanged value is suppressed (RBE)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn filter_deadband_passes_a_big_move() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let mk = |v: Value| {
        flow(
            "ws",
            "db",
            vec![mknode(
                "f",
                "filter",
                &[],
                with_payload(v),
                json!({"mode": "deadband", "deadband": 0.5}),
            )],
        )
    };
    save(&node, &p, "ws", &mk(json!(20.0)).clone()).await;
    assert_eq!(
        outcome(&fire(&node, &p, "ws", "db", "db-1", 1).await, "f"),
        "ok"
    );
    // 20.2 is within the 0.5 deadband → suppressed.
    save(&node, &p, "ws", &mk(json!(20.2))).await;
    assert_eq!(
        outcome(&fire(&node, &p, "ws", "db", "db-2", 2).await, "f"),
        "skipped"
    );
    // 21.0 moved > 0.5 → passes.
    save(&node, &p, "ws", &mk(json!(21.0))).await;
    assert_eq!(
        outcome(&fire(&node, &p, "ws", "db", "db-3", 3).await, "f"),
        "ok"
    );
}

// ---------------- Tier B: batch ----------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn batch_releases_at_the_count_boundary() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let mk = |v: Value| {
        flow(
            "ws",
            "bat",
            vec![mknode(
                "b",
                "batch",
                &[],
                with_payload(v),
                json!({"count": 2}),
            )],
        )
    };
    save(&node, &p, "ws", &mk(json!("a"))).await;
    let s1 = fire(&node, &p, "ws", "bat", "bat-1", 1).await;
    assert_eq!(outcome(&s1, "b"), "skipped", "first buffers, suppresses");
    save(&node, &p, "ws", &mk(json!("b"))).await;
    let s2 = fire(&node, &p, "ws", "bat", "bat-2", 2).await;
    assert_eq!(outcome(&s2, "b"), "ok", "second reaches count → releases");
    assert_eq!(
        output(&s2, "b")["payload"],
        json!(["a", "b"]),
        "grouped array, order preserved"
    );
}

// ---------------- Tier B: unique stream ----------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn unique_stream_dedupes_across_firings() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let mk = |v: Value| {
        flow(
            "ws",
            "uniq",
            vec![mknode(
                "u",
                "unique",
                &[],
                with_payload(v),
                json!({"mode": "stream"}),
            )],
        )
    };
    save(&node, &p, "ws", &mk(json!("A"))).await;
    assert_eq!(
        outcome(&fire(&node, &p, "ws", "uniq", "u-1", 1).await, "u"),
        "ok"
    );
    save(&node, &p, "ws", &mk(json!("A"))).await;
    assert_eq!(
        outcome(&fire(&node, &p, "ws", "uniq", "u-2", 2).await, "u"),
        "skipped",
        "dup dropped"
    );
    save(&node, &p, "ws", &mk(json!("B"))).await;
    assert_eq!(
        outcome(&fire(&node, &p, "ws", "uniq", "u-3", 3).await, "u"),
        "ok",
        "new key passes"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn unique_array_mode_dedupes_elements() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let f = flow(
        "ws",
        "ua",
        vec![mknode(
            "u",
            "unique",
            &[],
            with_payload(json!([1, 1, 2, 3, 3])),
            json!({}),
        )],
    );
    save(&node, &p, "ws", &f).await;
    let s = fire(&node, &p, "ws", "ua", "ua-1", 1).await;
    assert_eq!(output(&s, "u")["payload"], json!([1, 2, 3]));
}

// ---------------- Mandatory: workspace-isolation of the accumulator ----------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_isolation_batch_accumulator() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p1 = principal("ws1", FULL);
    let p2 = principal("ws2", FULL);
    // The SAME flow id, saved in both workspaces, batch count=2.
    let mk = |ws: &str, v: Value| {
        flow(
            ws,
            "iso",
            vec![mknode(
                "b",
                "batch",
                &[],
                with_payload(v),
                json!({"count": 2}),
            )],
        )
    };
    save(&node, &p1, "ws1", &mk("ws1", json!("x")).clone()).await;
    save(&node, &p2, "ws2", &mk("ws2", json!("y"))).await;
    // ws1 fires ONCE → buffers one item (suppressed, 1 in ws1's accumulator).
    let s1 = fire(&node, &p1, "ws1", "iso", "iso-w1", 1).await;
    assert_eq!(outcome(&s1, "b"), "skipped", "ws1 buffers its first item");
    // ws2 fires ONCE → if it could see ws1's buffered item it would reach count=2 and release. It
    // must NOT — ws2's accumulator is empty (the `{ws}:` wall), so ws2 also just buffers (suppresses).
    let s2 = fire(&node, &p2, "ws2", "iso", "iso-w2", 1).await;
    assert_eq!(
        outcome(&s2, "b"),
        "skipped",
        "ws2's batch is empty — ws1's accumulator is invisible across the workspace wall"
    );
}

// ---------------- Tier C: switch edge-gating ----------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn switch_fires_only_the_matched_port() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // sw routes 10 (>5) to `hi` only; `lo` (the else branch) must NOT run.
    let sw = mknode(
        "sw",
        "switch",
        &[],
        with_payload(json!(10)),
        json!({"rules": [
            {"op": "gt", "value": 5, "to": ["hi"]},
            {"op": "else", "to": ["lo"]}
        ]}),
    );
    let hi = mknode("hi", "count", &["sw"], Map::new(), json!({}));
    let lo = mknode("lo", "count", &["sw"], Map::new(), json!({}));
    let f = flow("ws", "sw", vec![sw, hi, lo]);
    save(&node, &p, "ws", &f).await;
    let s = fire(&node, &p, "ws", "sw", "sw-1", 1).await;
    assert_eq!(outcome(&s, "sw"), "ok");
    assert_eq!(outcome(&s, "hi"), "ok", "matched branch runs");
    assert_ne!(outcome(&s, "lo"), "ok", "unmatched branch does NOT run");
    assert_eq!(
        outcome(&s, "lo"),
        "skipped",
        "unmatched branch is gated (skipped)"
    );
}

// ---------------- Tier C: split → join round-trip ----------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn split_join_round_trips_an_array() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let split = mknode(
        "split",
        "split",
        &[],
        with_payload(json!([1, 2, 3])),
        json!({}),
    );
    let join = mknode("join", "join", &["split"], Map::new(), json!({}));
    let f = flow("ws", "sj", vec![split, join]);
    save(&node, &p, "ws", &f).await;
    let s = fire(&node, &p, "ws", "sj", "sj-1", 1).await;
    assert_eq!(
        output(&s, "split")["parts"]["count"],
        json!(3),
        "parts stamped"
    );
    assert_eq!(
        output(&s, "join")["payload"],
        json!([1, 2, 3]),
        "round-trips, order preserved"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn split_map_join_transforms_each_element() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // split an array of objects → map sets a field on each → join recombines (parts carries the
    // sequence through the map; join consumes it).
    let split = mknode(
        "split",
        "split",
        &[],
        with_payload(json!([{"id": 1}, {"id": 2}])),
        json!({}),
    );
    let map = mknode(
        "map",
        "map",
        &["split"],
        Map::new(),
        json!({"ops": [{"op": "set", "path": "ok", "value": true}]}),
    );
    let join = mknode("join", "join", &["map"], Map::new(), json!({}));
    let f = flow("ws", "smj", vec![split, map, join]);
    save(&node, &p, "ws", &f).await;
    let s = fire(&node, &p, "ws", "smj", "smj-1", 1).await;
    assert_eq!(
        output(&s, "join")["payload"],
        json!([{"id": 1, "ok": true}, {"id": 2, "ok": true}]),
        "each element transformed, sequence reassembled"
    );
}

// ---------------- Tier C: delay parks + resumes across a simulated restart ----------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn delay_parks_then_resumes_after_restart() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // delay 1000ms, then a count node downstream to prove release fires the frontier.
    let d = mknode(
        "d",
        "delay",
        &[],
        with_payload(json!([1, 2, 3])),
        json!({"mode": "delay", "ms": 1000}),
    );
    let sink = mknode("c", "count", &["d"], Map::new(), json!({}));
    let f = flow("ws", "dly", vec![d, sink]);
    save(&node, &p, "ws", &f).await;
    // Fire at t=1: the delay parks (release_at = 1001), the run suspends; the downstream count has
    // NOT run yet.
    let s1 = fire(&node, &p, "ws", "dly", "dly-1", 1).await;
    assert_eq!(s1["status"], "suspended", "delay parks → run suspends");
    assert_ne!(
        outcome(&s1, "c"),
        "ok",
        "downstream has not run while parked"
    );
    // Simulated restart: the durable release_at is in the store. Resume at t=2000 (> release_at) —
    // the delay releases and the run completes (never an in-memory sleep — the park was durable).
    let req = json!({ "run_id": "dly-1", "ts": 2000 }).to_string();
    call_tool(&node, &p, "ws", "flows.resume", &req)
        .await
        .unwrap();
    let s2 = await_status(&node, &p, "ws", "dly-1").await;
    assert_eq!(
        s2["status"], "success",
        "resume past the timer completes the run"
    );
    assert_eq!(outcome(&s2, "d"), "ok", "delay released");
    assert_eq!(
        output(&s2, "c")["payload"],
        json!(3),
        "downstream ran after release"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn delay_rate_limit_releases_when_spacing_elapses() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let mk = |v: Value| {
        flow(
            "ws",
            "rate",
            vec![mknode(
                "d",
                "delay",
                &[],
                with_payload(v),
                json!({"mode": "rate", "rate_ms": 100}),
            )],
        )
    };
    save(&node, &p, "ws", &mk(json!("a"))).await;
    // First firing releases immediately (no prior release).
    let s1 = fire(&node, &p, "ws", "rate", "rate-1", 0).await;
    assert_eq!(outcome(&s1, "d"), "ok", "first release is immediate");
    // Second firing at t=10 (< last+100) parks.
    save(&node, &p, "ws", &mk(json!("b"))).await;
    let s2 = fire(&node, &p, "ws", "rate", "rate-2", 10).await;
    assert_eq!(s2["status"], "suspended", "within the spacing → parks");
    // Resume at t=200 (> last+100) → releases.
    let req = json!({ "run_id": "rate-2", "ts": 200 }).to_string();
    call_tool(&node, &p, "ws", "flows.resume", &req)
        .await
        .unwrap();
    let s3 = await_status(&node, &p, "ws", "rate-2").await;
    assert_eq!(outcome(&s3, "d"), "ok", "release once the spacing elapses");
}
