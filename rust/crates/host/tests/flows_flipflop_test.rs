//! Host-layer tests for the `flipflop` source node (flip-flop-node-scope Testing plan). A self-driving
//! boolean oscillator: no input, one output, flips `true`/`false` every `period_secs`. Real store
//! (`mem://`) + real `lb-jobs` + real caps — no mocks. Injected clock via the logical `now` (never
//! wall-clock). Mandatory: capability-deny, workspace-isolation, the oscillation itself
//! (fire-once-then-skip + idempotent re-scan), and restart parity (value survives a store round-trip).

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_flows::{Flow, Node, Placement};
use lb_host::{call_tool, flipflop_run_id, react_to_flows_interval, Node as HostNode};
use lb_store::read as store_read;
use serde_json::{json, Value};

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:test".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    verify(&key, &mint(&key, &claims), 1).unwrap()
}

const FULL: &[&str] = &[
    "mcp:flows.save:call",
    "mcp:flows.get:call",
    "mcp:flows.run:call",
    "mcp:flows.runs.get:call",
    "mcp:rules.run:call",
    "mcp:rules.eval:call",
    "store:flow:write",
    "store:flow:read",
];

/// A canvas-authored flip-flop flow: a `flipflop` source (period 10s, start=true) feeding a `count`
/// node so the emitted boolean lands as a recorded, observable run output.
fn flipflop_flow(id: &str, node_id: &str, period_secs: u64, start: bool) -> Flow {
    let trig = Node {
        id: node_id.into(),
        node_type: "flipflop".into(),
        needs: vec![],
        with: Default::default(),
        config: json!({ "period_secs": period_secs, "start": start }),
    };
    // A rhai node that echoes the trigger's payload through, so the run records the boolean value.
    let echo = Node {
        id: "echo".into(),
        node_type: "rhai".into(),
        needs: vec![node_id.to_string()],
        with: Default::default(),
        config: json!({ "source": "payload" }),
    };
    Flow {
        workspace: "ws".into(),
        id: id.into(),
        name: id.into(),
        version: 0,
        params: Default::default(),
        nodes: vec![trig, echo],
        failure_policy: Default::default(),
        deleted: false,
        enabled: true,
        start_on_boot: false,
        placement: Placement::Either,
        concurrency: Default::default(),
        cron: None,
        next_attempt_ts: 0,
    }
}

async fn save(node: &Arc<HostNode>, p: &Principal, ws: &str, f: &Flow) {
    let body = serde_json::to_value(f).unwrap().to_string();
    call_tool(node, p, ws, "flows.save", &body).await.unwrap();
}

/// The per-node cursor (`flow_trigger_state:{flow}:{node}`).
async fn cursor(node: &Arc<HostNode>, ws: &str, flow: &str, node_id: &str) -> Value {
    store_read(
        &node.store,
        ws,
        "flow_trigger_state",
        &format!("{flow}:{node_id}"),
    )
    .await
    .unwrap()
    .unwrap_or(Value::Null)
}

/// Poll a fired run until terminal and return the boolean the `flipflop` emitted (echoed by `echo`).
async fn fired_value(node: &Arc<HostNode>, p: &Principal, ws: &str, run_id: &str) -> Value {
    for _ in 0..600 {
        let out = call_tool(
            node,
            p,
            ws,
            "flows.runs.get",
            &json!({ "run_id": run_id }).to_string(),
        )
        .await
        .unwrap();
        let snap: Value = serde_json::from_str(&out).unwrap();
        if snap["status"].as_str().unwrap_or("") == "success" {
            return snap["steps"]
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["id"] == "echo")
                .map(|s| s["output"]["payload"].clone())
                .unwrap_or(Value::Null);
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!("flip-flop run {run_id} never settled");
}

/// Advance the reactor to the node's due instant and fire once; returns the emitted boolean.
async fn tick_and_read(
    node: &Arc<HostNode>,
    p: &Principal,
    ws: &str,
    flow: &str,
    node_id: &str,
) -> Value {
    let next = cursor(node, ws, flow, node_id).await["next_attempt_ts"]
        .as_u64()
        .unwrap();
    let pass = react_to_flows_interval(node, p, ws, next).await.unwrap();
    assert_eq!(pass.fired, 1, "the flip-flop fired on its due instant");
    let run_id = flipflop_run_id(flow, node_id, next);
    fired_value(node, p, ws, &run_id).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn flipflop_oscillates_true_false_true() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    save(&node, &p, "ws", &flipflop_flow("osc", "ff", 10, true)).await;

    // First pass primes the cursor (no fire on init) at `now`, so the FIRST value fires next pass.
    react_to_flows_interval(&node, &p, "ws", 100).await.unwrap();
    assert_eq!(
        cursor(&node, "ws", "osc", "ff").await["next_attempt_ts"],
        100
    );

    // Three successive due firings flip: start(true) → false → true.
    assert_eq!(
        tick_and_read(&node, &p, "ws", "osc", "ff").await,
        json!(true)
    );
    assert_eq!(
        tick_and_read(&node, &p, "ws", "osc", "ff").await,
        json!(false)
    );
    assert_eq!(
        tick_and_read(&node, &p, "ws", "osc", "ff").await,
        json!(true)
    );

    // The cursor advanced by period each time (100 → 110 → 120 → 130).
    assert_eq!(
        cursor(&node, "ws", "osc", "ff").await["next_attempt_ts"],
        130
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn flipflop_re_scan_is_idempotent_no_double_flip() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    save(&node, &p, "ws", &flipflop_flow("idem", "ff", 10, true)).await;
    react_to_flows_interval(&node, &p, "ws", 100).await.unwrap(); // prime at 100

    // Fire the first instant; the cursor advances past `now` (100 → 110).
    let pass = react_to_flows_interval(&node, &p, "ws", 100).await.unwrap();
    assert_eq!(pass.fired, 1);
    // A re-scan at the SAME `now` is a no-op — the advanced cursor (110) is not yet due, so no second
    // flip. (The cursor advance IS the idempotency guard, exactly as the cron reactor: one instant, one
    // fire, one advance — a re-scan at the same time can never double-fire.)
    let pass2 = react_to_flows_interval(&node, &p, "ws", 100).await.unwrap();
    assert_eq!(pass2.fired, 0);
    // The stored value is still the first-fired value (`true`), not flipped by the re-scan.
    assert_eq!(cursor(&node, "ws", "idem", "ff").await["flop"], json!(true));
    assert_eq!(
        cursor(&node, "ws", "idem", "ff").await["next_attempt_ts"],
        110
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn flipflop_value_survives_a_store_round_trip() {
    // Restart parity: fire twice, then a fresh reactor pass (the cursor is the only durable state)
    // continues from the PERSISTED side, not from `start`.
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    save(&node, &p, "ws", &flipflop_flow("rp", "ff", 10, true)).await;
    react_to_flows_interval(&node, &p, "ws", 100).await.unwrap(); // prime
    assert_eq!(
        tick_and_read(&node, &p, "ws", "rp", "ff").await,
        json!(true)
    ); // 100 → true
    assert_eq!(
        tick_and_read(&node, &p, "ws", "rp", "ff").await,
        json!(false)
    ); // 110 → false

    // The durable cursor holds the last value `false`; the next firing must flip to `true`, proving the
    // value was read from the store (not reset to `start`).
    assert_eq!(cursor(&node, "ws", "rp", "ff").await["flop"], json!(false));
    assert_eq!(
        tick_and_read(&node, &p, "ws", "rp", "ff").await,
        json!(true)
    ); // 120 → true
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn flipflop_start_false_emits_false_first() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    save(&node, &p, "ws", &flipflop_flow("sf", "ff", 10, false)).await;
    react_to_flows_interval(&node, &p, "ws", 100).await.unwrap();
    assert_eq!(
        tick_and_read(&node, &p, "ws", "sf", "ff").await,
        json!(false)
    );
    assert_eq!(
        tick_and_read(&node, &p, "ws", "sf", "ff").await,
        json!(true)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn flipflop_disabled_flow_never_fires() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let mut f = flipflop_flow("off", "ff", 10, true);
    f.enabled = false;
    save(&node, &p, "ws", &f).await;
    // Even far past any instant, a disabled flow's flip-flop never fires (and no cursor is primed).
    let pass = react_to_flows_interval(&node, &p, "ws", 10_000)
        .await
        .unwrap();
    assert_eq!(pass.fired, 0);
    assert!(cursor(&node, "ws", "off", "ff").await.is_null());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn flipflop_workspace_isolation() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let pa = principal("ws-a", FULL);
    save(&node, &pa, "ws-a", &flipflop_flow("iso", "ff", 10, true)).await;
    // Prime + fire in ws-A so ws-A has a durable `flop=true` cursor.
    react_to_flows_interval(&node, &pa, "ws-a", 100)
        .await
        .unwrap();
    react_to_flows_interval(&node, &pa, "ws-a", 100)
        .await
        .unwrap();
    assert_eq!(
        cursor(&node, "ws-a", "iso", "ff").await["flop"],
        json!(true)
    );

    // A ws-B reactor pass never sees/fires ws-A's flow (the flow directory is ws-scoped).
    let pb = principal("ws-b", FULL);
    let pass = react_to_flows_interval(&node, &pb, "ws-b", 200)
        .await
        .unwrap();
    assert_eq!(pass.fired, 0);
    // And the SAME flow id run in ws-B (were it seeded there) would start fresh — ws-B has no cursor.
    assert!(cursor(&node, "ws-b", "iso", "ff").await.is_null());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn flipflop_capability_deny_no_run_no_state() {
    // No NEW capability: a `flipflop` fires a `flows.run` under the flow owner's authority, gated by the
    // existing `mcp:flows.run:call` at the MCP bridge — exactly the cron deny path. A caller WITHOUT
    // that cap who tries to drive the flow through the bridge is denied, and no run record is written.
    // (The reactor itself runs under the node's own system principal, which always holds the cap — the
    // deny boundary is the user-facing bridge, not the internal reactor call.)
    let node = Arc::new(HostNode::boot().await.unwrap());
    let saver = principal("ws", FULL);
    save(&node, &saver, "ws", &flipflop_flow("den", "ff", 10, true)).await;

    let caps: Vec<&str> = FULL
        .iter()
        .filter(|c| **c != "mcp:flows.run:call")
        .cloned()
        .collect();
    let denied = principal("ws", &caps);
    let err = call_tool(
        &node,
        &denied,
        "ws",
        "flows.run",
        &json!({ "id": "den", "run_id": "den-1", "ts": 100 }).to_string(),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, lb_mcp::ToolError::Denied));
    assert!(store_read(&node.store, "ws", "flow_run", "den-1")
        .await
        .unwrap()
        .is_none());
}
