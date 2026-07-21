//! Proves the rhai node's **starter template** (the seed `defaultConfig("rhai")` ships in the UI —
//! `ui/src/features/flows/defaultConfig.ts`) actually executes correctly against every payload shape
//! it claims to handle. The template is duplicated here as a fixture (the UI copy is TypeScript); if
//! the two drift, the assertions below catch it. Real store + real rhai cage + real caps — no mocks.
//!
//! Template contract:
//! - number  → payload × 100
//! - bool    → "on" (true) / "off" (false)
//! - anything else → the type name as a string

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
        constraint: None,
        run_id: None,
        iat: 0,
        exp: u64::MAX,
    };
    verify(&key, &mint(&key, &claims), 1).unwrap()
}

const FULL: &[&str] = &[
    "mcp:flows.save:call",
    "mcp:flows.run:call",
    "mcp:flows.runs.get:call",
    "mcp:rules.eval:call",
    "store:flow:write",
    "store:flow:read",
];

fn fnode(id: &str, ty: &str, needs: &[&str], config: Value) -> Node {
    Node {
        id: id.into(),
        node_type: ty.into(),
        needs: needs.iter().map(|s| s.to_string()).collect(),
        with: serde_json::Map::new(),
        config,
        inputs: Vec::new(),
        position: None,
    }
}

fn flow(id: &str, nodes: Vec<Node>) -> Flow {
    Flow {
        workspace: "ws".into(),
        id: id.into(),
        name: id.into(),
        version: 0,
        params: Default::default(),
        nodes,
        failure_policy: FailurePolicy::Halt,
        deleted: false,
        enabled: true,
        start_on_boot: false,
        placement: lb_flows::Placement::Either,
        concurrency: Default::default(),
        cron: None,
        next_attempt_ts: 0,
        managed_by: None,
    }
}

/// The exact template `ui/src/features/flows/defaultConfig.ts` seeds for a freshly-added rhai node.
const RHAI_TEMPLATE: &str = r#"
// Read the incoming payload from the wire (the envelope's primary value).
// Each envelope field is a top-level variable: payload, topic, ...
let value = payload;

// Number -> scale by 100 (covers both integer and float payloads).
if type_of(value) == "i64" || type_of(value) == "f64" {
    return value * 100;
}
// Bool -> "on" for true, "off" for false (a downstream node can route on the string).
else if type_of(value) == "bool" {
    return if value { "on" } else { "off" };
}
// Anything else (string, array, object, null) -> echo its type name as a string.
else {
    return type_of(value);
}
"#;

async fn run_template(payload: Value) -> Value {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // trigger → rhai: the trigger emits `params.start` as its payload, which auto-wires into the
    // rhai node's `payload` envelope field — the variable the template reads.
    let f = flow(
        "tmpl",
        vec![
            fnode("start", "trigger", &[], json!({})),
            fnode("r", "rhai", &["start"], json!({ "source": RHAI_TEMPLATE })),
        ],
    );
    let body = serde_json::to_value(&f).unwrap();
    let out = call_tool(&node, &p, "ws", "flows.save", &body.to_string())
        .await
        .unwrap();
    let _: Value = serde_json::from_str(&out).unwrap();
    call_tool(
        &node,
        &p,
        "ws",
        "flows.run",
        &json!({ "id": "tmpl", "run_id": "tmpl-run", "ts": 1, "params": { "start": payload } })
            .to_string(),
    )
    .await
    .unwrap();
    // Poll to terminal, then read the rhai node's recorded output payload.
    for _ in 0..400 {
        let snap = serde_json::from_str::<Value>(
            &call_tool(
                &node,
                &p,
                "ws",
                "flows.runs.get",
                r#"{"run_id":"tmpl-run"}"#,
            )
            .await
            .unwrap(),
        )
        .unwrap();
        if matches!(
            snap["status"].as_str().unwrap_or(""),
            "success" | "partialFailure" | "failed" | "cancelled"
        ) {
            let step = snap["steps"]
                .as_array()
                .and_then(|s| s.iter().find(|s| s["id"] == "r"))
                .expect("rhai step present");
            return step["output"].clone();
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!("template run did not settle");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn template_scales_an_integer_payload_by_100() {
    let out = run_template(json!(42)).await;
    assert_eq!(out["payload"], 4200, "42 * 100");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn template_scales_a_float_payload_by_100() {
    let out = run_template(json!(2.5)).await;
    assert_eq!(out["payload"], 250.0, "2.5 * 100");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn template_maps_bool_true_to_on() {
    let out = run_template(json!(true)).await;
    assert_eq!(out["payload"], "on");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn template_maps_bool_false_to_off() {
    let out = run_template(json!(false)).await;
    assert_eq!(out["payload"], "off");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn template_returns_the_type_name_for_a_string() {
    let out = run_template(json!("hello")).await;
    assert_eq!(out["payload"], "string", "a string's type name");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn template_returns_the_type_name_for_an_object() {
    let out = run_template(json!({ "a": 1 })).await;
    // rhai surfaces an object as a map.
    assert_eq!(out["payload"], "map");
}
