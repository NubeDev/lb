//! Host-layer tests for the `sink` flow node's terminal writes (real store `mem://`, real caps, real
//! ingest + inbox — no mocks). A `sink` is the one node that WRITES out of a flow: `series`→
//! `ingest.write`, `inbox`/`channel`→`inbox.record`, `outbox`→the outbox. The regression this guards:
//! the sink built request shapes that did NOT match the target verbs' contracts — `series` sent
//! `{series,value,ts}` (the `Sample` shape needs `producer`/`seq`/`payload`, failing with "missing
//! field `producer`") and `inbox` sent `{channel,body}` (no `id`, failing with "missing arg: id"). So a
//! sink could never write. These drive a real run and then READ the target back to prove the value
//! landed.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_flows::{FailurePolicy, Flow, Node};
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

/// Flows write + run caps plus the sink targets' verbs (ingest/series/inbox).
const CAPS: &[&str] = &[
    "mcp:flows.save:call",
    "mcp:flows.run:call",
    "mcp:flows.runs.get:call",
    "store:flow:write",
    "store:flow:read",
    "mcp:ingest.write:call",
    "mcp:series.latest:call",
    "mcp:inbox.record:call",
    "mcp:inbox.list:call",
];

fn flow_with(id: &str, sink: Node) -> Flow {
    Flow {
        workspace: "ws".into(),
        id: id.into(),
        name: id.into(),
        version: 1,
        params: serde_json::Map::new(),
        nodes: vec![sink],
        failure_policy: FailurePolicy::Halt,
        deleted: false,
        enabled: true,
        start_on_boot: false,
        placement: Default::default(),
        cron: None,
        next_attempt_ts: 0,
    }
}

/// A lone `sink` fed a literal `value` via `with` (the host resolves the literal unchanged) — the same
/// pattern the `count_node_counts_its_input` test uses for a lone transform.
fn sink_node(target: &str, name: &str, value: Value) -> Node {
    Node {
        id: "out".into(),
        node_type: "sink".into(),
        needs: vec![],
        with: serde_json::Map::from_iter([("value".into(), value)]),
        config: json!({ "target": target, "name": name }),
    }
}

async fn save_flow(node: &Arc<HostNode>, p: &Principal, f: &Flow) {
    let body = serde_json::to_value(f).unwrap();
    call_tool(node, p, "ws", "flows.save", &body.to_string())
        .await
        .unwrap();
}

async fn run_to_terminal(node: &Arc<HostNode>, p: &Principal, id: &str, run_id: &str) -> Value {
    let req = json!({ "id": id, "run_id": run_id, "ts": 7 }).to_string();
    call_tool(node, p, "ws", "flows.run", &req).await.unwrap();
    for _ in 0..400 {
        let out = call_tool(
            node,
            p,
            "ws",
            "flows.runs.get",
            &json!({ "run_id": run_id }).to_string(),
        )
        .await
        .unwrap();
        let snap: Value = serde_json::from_str(&out).unwrap();
        let st = snap["status"].as_str().unwrap_or("");
        if matches!(st, "success" | "failed" | "partialFailure" | "cancelled") {
            return snap;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!("run {run_id} did not settle");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn sink_series_writes_a_sample_readable_by_series_latest() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", CAPS);

    let f = flow_with(
        "sink-series",
        sink_node("series", "flow.sink.series", json!(42)),
    );
    save_flow(&node, &p, &f).await;
    let snap = run_to_terminal(&node, &p, "sink-series", "sink-series-1").await;

    // The sink no longer errors ("missing field producer") — the step settles ok with {accepted:1}.
    assert_eq!(snap["status"], "success", "run snapshot: {snap}");
    assert_eq!(
        snap["steps"][0]["outcome"], "ok",
        "sink step: {}",
        snap["steps"][0]
    );
    assert_eq!(snap["steps"][0]["output"]["accepted"], 1);

    // And the value actually landed in the series (the round-trip the proof the write is real).
    let out = call_tool(
        &node,
        &p,
        "ws",
        "series.latest",
        &json!({ "series": "flow.sink.series" }).to_string(),
    )
    .await
    .unwrap();
    let latest: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        latest["sample"]["payload"],
        json!(42),
        "series.latest: {latest}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn sink_inbox_records_an_item_readable_by_inbox_list() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", CAPS);

    let f = flow_with(
        "sink-inbox",
        sink_node("inbox", "alerts", json!("hello from a flow")),
    );
    save_flow(&node, &p, &f).await;
    let snap = run_to_terminal(&node, &p, "sink-inbox", "sink-inbox-1").await;

    // No more "missing arg: id" — the step settles ok with {recorded:true}.
    assert_eq!(snap["status"], "success", "run snapshot: {snap}");
    assert_eq!(
        snap["steps"][0]["outcome"], "ok",
        "sink step: {}",
        snap["steps"][0]
    );
    assert_eq!(snap["steps"][0]["output"]["recorded"], true);

    // The item is in the channel with the flow's value as its body.
    let out = call_tool(
        &node,
        &p,
        "ws",
        "inbox.list",
        &json!({ "channel": "alerts" }).to_string(),
    )
    .await
    .unwrap();
    let listed: Value = serde_json::from_str(&out).unwrap();
    let items = listed["items"].as_array().expect("items array");
    assert!(
        items
            .iter()
            .any(|it| it["body"] == json!("hello from a flow")),
        "inbox.list did not contain the flow's item: {listed}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn sink_inbox_records_a_structured_value_as_a_stringified_body() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", CAPS);

    // A structured value must not silently become an empty body — it's stringified.
    let f = flow_with(
        "sink-inbox2",
        sink_node("inbox", "alerts2", json!({ "count": 5 })),
    );
    save_flow(&node, &p, &f).await;
    let snap = run_to_terminal(&node, &p, "sink-inbox2", "sink-inbox2-1").await;
    assert_eq!(snap["status"], "success", "run snapshot: {snap}");

    let out = call_tool(
        &node,
        &p,
        "ws",
        "inbox.list",
        &json!({ "channel": "alerts2" }).to_string(),
    )
    .await
    .unwrap();
    let listed: Value = serde_json::from_str(&out).unwrap();
    let items = listed["items"].as_array().expect("items array");
    assert!(
        items.iter().any(|it| it["body"]
            .as_str()
            .is_some_and(|b| b.contains("\"count\":5"))),
        "structured body not stringified: {listed}"
    );
}
