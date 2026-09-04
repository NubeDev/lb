//! The two MANDATORY categories for series-normalize at the host seam: **capability deny** (the
//! additive `filter`/`method` fields mint no new cap, so the existing `series.retention.set` /
//! `series.read` gates must already cover them) and **workspace isolation** (a ws-B policy filters
//! and folds nothing in ws-A).
//!
//! The wire contract itself is `series_normalize_test.rs`. Real node boot, real store, real MCP
//! dispatch — no mocks (testing §0).

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_ingest_tool, Node};
use lb_ingest::{Qos, Sample};
use lb_mcp::ToolError;
use serde_json::{json, Value};

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

/// An admin principal for `ws` — every series cap this suite touches.
fn admin(ws: &str) -> Principal {
    principal(
        "user:test",
        ws,
        &[
            "mcp:series.retention.set:call",
            "mcp:series.retention.list:call",
            "mcp:series.retention.gc:call",
            "mcp:series.read:call",
            "mcp:series.latest:call",
            "mcp:ingest.write:call",
        ],
    )
}

async fn call(
    node: &Node,
    p: &Principal,
    ws: &str,
    tool: &str,
    args: Value,
) -> Result<Value, ToolError> {
    call_ingest_tool(&node.store, p, ws, tool, &args).await
}

/// Commit `samples` through the real write → drain path.
async fn seed(node: &Node, ws: &str, samples: Vec<Sample>) {
    lb_ingest::commit_direct(&node.store, ws, &samples)
        .await
        .unwrap();
}

fn sample_at(series: &str, producer: &str, seq: u64, ts: u64, payload: Value) -> Sample {
    Sample {
        series: series.into(),
        producer: producer.into(),
        ts,
        seq,
        payload,
        labels: json!({}),
        qos: Qos::BestEffort,
    }
}

// ---------------------------------------------------------------- mandatory: capability deny ----

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn setting_a_filter_without_the_admin_cap_is_denied() {
    // The additive fields mint NO new cap — they ride `mcp:series.retention.set:call`. This proves
    // the existing gate actually covers them: a caller who may READ policies still cannot write one
    // carrying a filter, and the denial is opaque.
    let node = Node::boot().await.unwrap();
    let reader = principal(
        "user:bob",
        "nube",
        &["mcp:series.retention.list:call", "mcp:series.read:call"],
    );

    let err = call(
        &node,
        &reader,
        "nube",
        "series.retention.set",
        json!({
            "prefix": "modbus.",
            "raw_for_ms": 900000,
            "filter": {"deadband": {"abs": 0.5}},
            "tiers": [{"width_ms": 900000, "keep_for_ms": 0, "method": "avg"}]
        }),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ToolError::Denied),
        "expected opaque Denied, got {err:?}"
    );

    // And nothing was written — a denial must not half-apply.
    let listed = call(&node, &reader, "nube", "series.retention.list", json!({}))
        .await
        .unwrap();
    assert_eq!(listed["policies"].as_array().unwrap().len(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_bucketed_read_with_a_method_still_needs_the_read_cap() {
    let node = Node::boot().await.unwrap();
    let no_read = principal("user:bob", "nube", &["mcp:series.retention.list:call"]);
    let err = call(
        &node,
        &no_read,
        "nube",
        "series.read",
        json!({"series": "m.v", "mode": "buckets", "from": 0, "to": 10000, "width_ms": 1000, "method": "avg"}),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied), "got {err:?}");
}

// ------------------------------------------------------------ mandatory: workspace isolation ----

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_ws_b_policy_filters_and_folds_nothing_in_ws_a() {
    let node = Node::boot().await.unwrap();

    // ws-B mutes `temp.` outright and declares a `last` tier. ws-A declares nothing.
    call(
        &node,
        &admin("beta"),
        "beta",
        "series.retention.set",
        json!({
            "prefix": "temp.",
            "raw_for_ms": 0,
            "filter": {"drop": true},
            "tiers": [{"width_ms": 10000, "keep_for_ms": 0, "method": "last"}]
        }),
    )
    .await
    .unwrap();

    let rows = |p: &str| {
        vec![
            sample_at("temp.a", p, 1, 1_000, json!(1.0)),
            sample_at("temp.a", p, 2, 2_000, json!(2.0)),
        ]
    };
    seed(&node, "nube", rows("pa")).await;
    seed(&node, "beta", rows("pb")).await;

    // ws-A is untouched by ws-B's mute…
    let a = call(
        &node,
        &admin("nube"),
        "nube",
        "series.read",
        json!({"series": "temp.a"}),
    )
    .await
    .unwrap();
    assert_eq!(
        a["samples"].as_array().unwrap().len(),
        2,
        "ws-A stored everything"
    );

    // …and ws-B's own policy applied only inside ws-B.
    let b = call(
        &node,
        &admin("beta"),
        "beta",
        "series.read",
        json!({"series": "temp.a"}),
    )
    .await
    .unwrap();
    assert_eq!(
        b["samples"].as_array().unwrap().len(),
        0,
        "ws-B muted its own"
    );

    // ws-A cannot see ws-B's policy at all.
    let a_policies = call(
        &node,
        &admin("nube"),
        "nube",
        "series.retention.list",
        json!({}),
    )
    .await
    .unwrap();
    assert_eq!(a_policies["policies"].as_array().unwrap().len(), 0);

    // And ws-A's bucketed read resolves NO method — ws-B's `last` tier does not govern it.
    let a_buckets = call(
        &node,
        &admin("nube"),
        "nube",
        "series.read",
        json!({"series": "temp.a", "mode": "buckets", "from": 0, "to": 10000, "width_ms": 10000}),
    )
    .await
    .unwrap();
    assert_eq!(
        a_buckets["method"],
        Value::Null,
        "no policy in ws-A → no method"
    );
    assert!(a_buckets["buckets"][0].get("value").is_none());
}

// ---------------------------------------------------------------------- the wire contract ----
