//! Tier-A integration tests for the data/JSON node pack (data-nodes scope) — the pure transform +
//! parse nodes, exercised end-to-end through the **real** `flows.save`/`flows.run` path on the real
//! store (`mem://`), real caps, real jobs (CLAUDE §9 — no mocks, no fakes). Each node is dropped into
//! a one-node flow with a literal `payload` binding; the run settles and we assert the emitted
//! `output.payload`. Failure cases (malformed csv/xml/yaml, invalid base64) assert the node records
//! `err` — the `json`-node parity.
//!
//! Mandatory categories here: **capability-deny** (a `flows.run` without `mcp:flows.run:call`
//! executes no node) and a slice of the Tier-A per-node table. Stateful/engine nodes + the
//! workspace-isolation accumulator test live in `flows_data_engine_test.rs`.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_flows::{FailurePolicy, Flow, Node, Placement};
use lb_host::{call_tool, Node as HostNode};
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
    "mcp:flows.run:call",
    "mcp:flows.resume:call",
    "mcp:flows.runs.get:call",
    "store:flow:write",
    "store:flow:read",
];

fn one_node_flow(id: &str, node_type: &str, config: Value, payload: Value) -> Flow {
    let node = Node {
        id: "n".into(),
        node_type: node_type.into(),
        needs: vec![],
        with: serde_json::Map::from_iter([("payload".into(), payload)]),
        config,
    };
    Flow {
        workspace: "ws".into(),
        id: id.into(),
        name: id.into(),
        version: 0,
        params: Default::default(),
        nodes: vec![node],
        failure_policy: FailurePolicy::Halt,
        deleted: false,
        enabled: true,
        start_on_boot: false,
        placement: Placement::Either,
        cron: None,
        next_attempt_ts: 0,
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

/// Poll until the run reaches a terminal status; return the snapshot.
async fn await_terminal(node: &Arc<HostNode>, p: &Principal, ws: &str, run_id: &str) -> Value {
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

/// Run a single-node flow and return that node's terminal `{outcome, output}`.
async fn run_one(node_type: &str, config: Value, payload: Value) -> (String, Value) {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let flow_id = format!("f_{node_type}");
    let f = one_node_flow(&flow_id, node_type, config, payload);
    save(&node, &p, "ws", &f).await;
    let run_id = format!("{flow_id}-r");
    let req = json!({ "id": flow_id, "run_id": run_id, "ts": 1 }).to_string();
    call_tool(&node, &p, "ws", "flows.run", &req).await.unwrap();
    let snap = await_terminal(&node, &p, "ws", &run_id).await;
    let step = &snap["steps"][0];
    (
        step["outcome"].as_str().unwrap().to_string(),
        step["output"].clone(),
    )
}

/// Convenience: assert a node emits `expected` as its `payload`.
async fn assert_payload(node_type: &str, config: Value, payload: Value, expected: Value) {
    let (outcome, output) = run_one(node_type, config, payload).await;
    assert_eq!(outcome, "ok", "{node_type} should settle ok");
    assert_eq!(output["payload"], expected, "{node_type} payload");
}

/// Convenience: assert a node FAILS (records `err`) — the malformed-input parity.
async fn assert_fails(node_type: &str, config: Value, payload: Value) {
    let (outcome, _) = run_one(node_type, config, payload).await;
    assert_eq!(
        outcome, "err",
        "{node_type} should fail the node on bad input"
    );
}

// ---------------- Data category ----------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn change_reshapes_the_payload() {
    assert_payload(
        "change",
        json!({"ops": [
            {"op": "set", "path": "b", "value": 2},
            {"op": "move", "from": "a", "to": "c"},
            {"op": "delete", "path": "junk"}
        ]}),
        json!({"a": 1, "junk": true}),
        json!({"b": 2, "c": 1}),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn select_keeps_only_chosen_paths() {
    assert_payload(
        "select",
        json!({"paths": ["a", "nested.x"]}),
        json!({"a": 1, "b": 2, "nested": {"x": 9, "y": 8}}),
        json!({"a": 1, "nested": {"x": 9}}),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn merge_deep_merges_an_array_of_objects() {
    assert_payload(
        "merge",
        json!({}),
        json!([{"a": 1, "n": {"x": 1}}, {"b": 2, "n": {"y": 2}}]),
        json!({"a": 1, "b": 2, "n": {"x": 1, "y": 2}}),
    )
    .await;
    assert_fails("merge", json!({}), json!("not an array")).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn map_applies_ops_per_element() {
    assert_payload(
        "map",
        json!({"ops": [{"op": "set", "path": "seen", "value": true}]}),
        json!([{"id": 1}, {"id": 2}]),
        json!([{"id": 1, "seen": true}, {"id": 2, "seen": true}]),
    )
    .await;
    assert_fails("map", json!({"ops": []}), json!({"not": "an array"})).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn flatten_arrays_and_objects() {
    assert_payload(
        "flatten",
        json!({}),
        json!([1, [2, [3, 4]]]),
        json!([1, 2, 3, 4]),
    )
    .await;
    assert_payload(
        "flatten",
        json!({}),
        json!({"a": {"b": 1, "c": 2}}),
        json!({"a.b": 1, "a.c": 2}),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn sort_by_field_numeric_and_lexical() {
    assert_payload(
        "sort",
        json!({"path": "v", "numeric": true, "order": "desc"}),
        json!([{"v": 2}, {"v": 10}, {"v": 1}]),
        json!([{"v": 10}, {"v": 2}, {"v": 1}]),
    )
    .await;
    assert_fails("sort", json!({}), json!("scalar")).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn range_scales_and_clamps() {
    // 512 in [0,1023] → [-40,125]; clamp keeps within output range.
    let (outcome, output) = run_one(
        "range",
        json!({"inMin": 0, "inMax": 1023, "outMin": -40, "outMax": 125, "clamp": true}),
        json!(512),
    )
    .await;
    assert_eq!(outcome, "ok");
    let v = output["payload"].as_f64().unwrap();
    assert!((v - 42.6).abs() < 0.5, "scaled ~42.6, got {v}");
    assert_fails(
        "range",
        json!({"inMin": 0, "inMax": 10, "outMin": 0, "outMax": 1}),
        json!("not a number"),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn aggregate_reduces_an_array() {
    assert_payload(
        "aggregate",
        json!({"op": "sum"}),
        json!([1, 2, 3]),
        json!(6.0),
    )
    .await;
    assert_payload(
        "aggregate",
        json!({"op": "max", "path": "v"}),
        json!([{"v": 3}, {"v": 9}, {"v": 1}]),
        json!(9.0),
    )
    .await;
    assert_payload(
        "aggregate",
        json!({"op": "count"}),
        json!([1, 2, 3, 4]),
        json!(4),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn template_renders_text() {
    assert_payload(
        "template",
        json!({"template": "temp={{temp}} at {{site.name}}"}),
        json!({"temp": 21, "site": {"name": "roof"}}),
        json!("temp=21 at roof"),
    )
    .await;
}

// ---------------- Parse category (malformed → fails) ----------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn csv_parse_and_stringify_and_fail() {
    assert_payload(
        "csv",
        json!({"mode": "parse"}),
        json!("a,b\n1,2"),
        json!([{"a": "1", "b": "2"}]),
    )
    .await;
    assert_payload(
        "csv",
        json!({"mode": "stringify"}),
        json!([{"a": "1", "b": "2"}]),
        json!("a,b\n1,2\n"),
    )
    .await;
    assert_fails("csv", json!({"mode": "parse"}), json!("a,b\n1,2,3")).await; // ragged
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn xml_round_trip_and_fail() {
    assert_payload(
        "xml",
        json!({"mode": "parse"}),
        json!("<r><a>1</a><a>2</a></r>"),
        json!({"r": {"a": ["1", "2"]}}),
    )
    .await;
    assert_fails("xml", json!({"mode": "parse"}), json!("<r><a></r>")).await; // unclosed
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn yaml_parse_and_fail() {
    assert_payload(
        "yaml",
        json!({"mode": "parse"}),
        json!("name: bob\ntags:\n  - x\n  - y\n"),
        json!({"name": "bob", "tags": ["x", "y"]}),
    )
    .await;
    assert_fails("yaml", json!({"mode": "parse"}), json!("a: [1, 2")).await; // unclosed flow seq
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn base64_encode_decode_and_fail() {
    assert_payload(
        "base64",
        json!({"mode": "encode"}),
        json!("hello"),
        json!("aGVsbG8="),
    )
    .await;
    assert_payload(
        "base64",
        json!({"mode": "decode"}),
        json!("aGVsbG8="),
        json!("hello"),
    )
    .await;
    assert_fails("base64", json!({"mode": "decode"}), json!("not*base64*")).await;
}

// ---------------- Sequence (split/join array-carry) ----------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn split_stamps_parts() {
    let (outcome, output) = run_one("split", json!({}), json!([10, 20, 30])).await;
    assert_eq!(outcome, "ok");
    assert_eq!(output["payload"], json!([10, 20, 30]));
    assert_eq!(output["parts"]["count"], json!(3));
    assert_eq!(output["parts"]["kind"], json!("array"));
}

// ---------------- Mandatory: capability-deny ----------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn capability_deny_run_without_flows_run_cap_executes_no_node() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let saver = principal("ws", FULL);
    // runner lacks `mcp:flows.run:call` — a data-pack flow is denied and NO node runs.
    let caps: Vec<&str> = FULL
        .iter()
        .filter(|c| **c != "mcp:flows.run:call")
        .cloned()
        .collect();
    let runner = principal("ws", &caps);
    let f = one_node_flow("deny", "change", json!({"ops": []}), json!({"a": 1}));
    save(&node, &saver, "ws", &f).await;
    let req = json!({ "id": "deny", "run_id": "deny-r", "ts": 1 }).to_string();
    let err = call_tool(&node, &runner, "ws", "flows.run", &req)
        .await
        .unwrap_err();
    assert!(
        matches!(err, lb_mcp::ToolError::Denied),
        "run must be denied"
    );
    // No run record was created → no node executed (nothing to settle): `flows.runs.get` is a
    // NotFound (an Err) or an empty snapshot.
    let got = call_tool(&node, &saver, "ws", "flows.runs.get", &req).await;
    if let Ok(out) = got {
        let snap: Value = serde_json::from_str(&out).unwrap();
        assert!(
            snap["steps"]
                .as_array()
                .map(|a| a.is_empty())
                .unwrap_or(true),
            "a denied run executed no node: {snap}"
        );
    }
}
