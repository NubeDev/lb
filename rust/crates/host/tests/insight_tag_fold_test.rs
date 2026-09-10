//! The **`Human > Producer` tag fold** on the real raise path
//! (`docs/scope/insights/insight-tag-precedence-scope.md`).
//!
//! `facets.rs` unit-tests the fold as a pure function. This file asserts the thing that actually
//! bit: an operator corrects a finding's classification, the nightly rule fires again, and the
//! correction **survives** — through the real store (`mem://`), the real tag graph, the real caps
//! and the real `insight.raise`, with the echo read back through `insight.get` AND `insight.list`
//! (a test that only checks `get` passes while the roster stays broken).
//!
//! No mocks (CLAUDE §9). Every tag edge is written through the real `lb_tags::add` — deliberately
//! NOT through the `tags.add` MCP verb, because that door is registered by the lead in the same
//! wave and this file must be able to go green on the fold alone (`tags_door_test.rs` owns the
//! door's own assertions).

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, Node};
use lb_mcp::ToolError;
use lb_tags::{Provenance, Source, Tag, DEFAULT_TAG_NODE_CAP};
use serde_json::{json, Value};

const RAISE: &str = "mcp:insight.raise:call";
const GET: &str = "mcp:insight.get:call";
const LIST: &str = "mcp:insight.list:call";

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

async fn call(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    tool: &str,
    input: Value,
) -> Result<Value, ToolError> {
    let out = call_tool(node, p, ws, tool, &input.to_string()).await?;
    Ok(serde_json::from_str(&out).unwrap())
}

fn raise_input(dedup_key: &str, ts: u64, tags: Value) -> Value {
    json!({
        "dedup_key": dedup_key,
        "severity": "warning",
        "title": "finding",
        "origin": { "kind": "rule", "ref": "rule:classify" },
        "tags": tags,
        "ts": ts,
    })
}

async fn raise_id(node: &Arc<Node>, p: &Principal, ws: &str, input: Value) -> String {
    call(node, p, ws, "insight.raise", input)
        .await
        .expect("raise ok")["id"]
        .as_str()
        .expect("id")
        .to_string()
}

/// Write one tag edge through the REAL graph, with an explicit source + provenance ts.
async fn edge(node: &Arc<Node>, ws: &str, id: &str, k: &str, v: &str, source: Source, at: u64) {
    lb_tags::add(
        &node.store,
        ws,
        &format!("insight:{id}"),
        &Tag::new(k.to_string(), json!(v)),
        &Provenance::new(at, "user:priya", source),
        DEFAULT_TAG_NODE_CAP,
    )
    .await
    .expect("edge added");
}

/// The echo a `list` page carries for `id` — the surface a roster actually renders.
async fn list_tags(node: &Arc<Node>, p: &Principal, ws: &str, id: &str) -> Value {
    let page = call(node, p, ws, "insight.list", json!({}))
        .await
        .expect("list ok");
    page["items"]
        .as_array()
        .expect("items")
        .iter()
        .find(|i| i["id"] == id)
        .map(|i| i["tags"].clone())
        .unwrap_or(Value::Null)
}

/// **THE REGRESSION.** A producer edge and a human edge coexist for one key (the graph's
/// `(entity, tag, source)` identity allows exactly this). The echo must carry the HUMAN value —
/// and must still carry it after the rule fires again, which is where last-write-wins reverted it.
///
/// Note the producer's edge is the NEWER one on the re-raise, which is not a contrivance: the
/// machine re-asserts on every firing, so under newest-wins the human always eventually loses.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_human_correction_survives_the_next_producer_raise() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    let p = principal("user:test", ws, &[RAISE, GET, LIST]);

    // 1. The rule raises and asserts its own classification (Producer, via the raise input).
    let id = raise_id(
        &node,
        &p,
        ws,
        raise_input(
            "ahu-2:classify",
            1_000,
            json!({ "classification": "plumbing" }),
        ),
    )
    .await;

    // 2. An operator disagrees and corrects it — an out-of-band Human edge on the same key.
    edge(
        &node,
        ws,
        &id,
        "classification",
        "mechanical",
        Source::Human,
        10,
    )
    .await;

    // 3. Tonight's firing re-asserts the producer value, at a strictly NEWER provenance ts.
    let same = raise_id(
        &node,
        &p,
        ws,
        raise_input(
            "ahu-2:classify",
            2_000,
            json!({ "classification": "plumbing" }),
        ),
    )
    .await;
    assert_eq!(same, id, "dedup key holds — this is the same finding");

    let got = call(&node, &p, ws, "insight.get", json!({ "id": &id }))
        .await
        .expect("get ok");
    assert_eq!(
        got["tags"]["classification"], "mechanical",
        "the operator's correction must survive the producer's re-assertion: {}",
        got["tags"]
    );

    // The roster column is the surface that actually shows this to a human.
    assert_eq!(
        list_tags(&node, &p, ws, &id).await["classification"],
        "mechanical",
        "the list page carries the folded value, not the producer's"
    );
}

/// **Determinism.** Two workspaces given the SAME two edges in the OPPOSITE insertion order must
/// echo identically. Two workspaces rather than two records so nothing else can differ; the bug the
/// fold replaced was precisely that this depended on the order `tags.of` happened to return.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn insertion_order_cannot_change_the_echo() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p_a = principal("user:test", "ws-a", &[RAISE, GET]);
    let p_b = principal("user:test", "ws-b", &[RAISE, GET]);

    let a = raise_id(&node, &p_a, "ws-a", raise_input("k", 1, json!({}))).await;
    edge(
        &node,
        "ws-a",
        &a,
        "classification",
        "plumbing",
        Source::Producer,
        20,
    )
    .await;
    edge(
        &node,
        "ws-a",
        &a,
        "classification",
        "mechanical",
        Source::Human,
        10,
    )
    .await;

    let b = raise_id(&node, &p_b, "ws-b", raise_input("k", 1, json!({}))).await;
    edge(
        &node,
        "ws-b",
        &b,
        "classification",
        "mechanical",
        Source::Human,
        10,
    )
    .await;
    edge(
        &node,
        "ws-b",
        &b,
        "classification",
        "plumbing",
        Source::Producer,
        20,
    )
    .await;

    // Re-raise both so the echo is re-materialized from the graph in each workspace.
    raise_id(&node, &p_a, "ws-a", raise_input("k", 2, json!({}))).await;
    raise_id(&node, &p_b, "ws-b", raise_input("k", 2, json!({}))).await;

    let ta = call(&node, &p_a, "ws-a", "insight.get", json!({ "id": &a }))
        .await
        .expect("get a")["tags"]
        .clone();
    let tb = call(&node, &p_b, "ws-b", "insight.get", json!({ "id": &b }))
        .await
        .expect("get b")["tags"]
        .clone();
    assert_eq!(ta, tb, "reversed edge order, identical echo");
    assert_eq!(ta["classification"], "mechanical");
}

/// Within one source the newest provenance wins — the tie-break that keeps a re-correction from
/// losing to the operator's own earlier value.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn within_one_source_the_newest_edge_wins() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    let p = principal("user:test", ws, &[RAISE, GET]);
    let id = raise_id(&node, &p, ws, raise_input("k", 1, json!({}))).await;

    // Two HUMAN edges for one key. Same source ⇒ the graph keeps both only if the values differ;
    // the fold has to pick the newer.
    edge(&node, ws, &id, "priority", "medium", Source::Human, 10).await;
    edge(&node, ws, &id, "priority", "high", Source::Human, 99).await;
    raise_id(&node, &p, ws, raise_input("k", 2, json!({}))).await;

    let got = call(&node, &p, ws, "insight.get", json!({ "id": &id }))
        .await
        .expect("get ok");
    assert_eq!(
        got["priority"]
            .as_str()
            .or(got["tags"]["priority"].as_str()),
        Some("high"),
        "newest provenance within one source: {}",
        got["tags"]
    );
}

/// **MANDATORY — workspace isolation.** The same key and the same values in two workspaces: the
/// fold is per-workspace, and a denormalized copy of graph data is exactly where cross-ws bleed
/// hides.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_fold_does_not_bleed_across_workspaces() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p_a = principal("user:test", "ws-a", &[RAISE, GET]);
    let p_b = principal("user:test", "ws-b", &[RAISE, GET]);

    let a = raise_id(
        &node,
        &p_a,
        "ws-a",
        raise_input("shared-key", 1, json!({ "classification": "plumbing" })),
    )
    .await;
    edge(
        &node,
        "ws-a",
        &a,
        "classification",
        "mechanical",
        Source::Human,
        10,
    )
    .await;
    raise_id(&node, &p_a, "ws-a", raise_input("shared-key", 2, json!({}))).await;

    let b = raise_id(
        &node,
        &p_b,
        "ws-b",
        raise_input("shared-key", 1, json!({ "classification": "plumbing" })),
    )
    .await;

    let got_b = call(&node, &p_b, "ws-b", "insight.get", json!({ "id": &b }))
        .await
        .expect("get b");
    assert_eq!(
        got_b["tags"]["classification"], "plumbing",
        "ws-b's echo is folded from ws-b's edges only — the human correction lives in ws-a"
    );

    // And ws-b cannot even see ws-a's record.
    let cross = call(&node, &p_b, "ws-b", "insight.get", json!({ "id": &a })).await;
    assert!(
        cross
            .map(|v| v.is_null() || v.get("id").is_none())
            .unwrap_or(true),
        "ws-b must not read ws-a's finding"
    );
}
