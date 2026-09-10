//! The **caveat stamp** and the **workspace category vocabulary**
//! (`docs/scope/insights/case-plane-scope.md` §"Data model" + §"Vocabulary").
//!
//! Two facts land together here because they read the same store row:
//!
//!   - **`category` is validated against the workspace's own declared set.** lb ships NO default
//!     value list (rule 10) — an unseeded workspace validates nothing, and a seeded one rejects a
//!     value outside its declared set, naming the set.
//!   - **A finding derived from data that is itself under question is caveated**, and a caveated
//!     finding **never breaks through**: no immediate delivery, while the identical uncaveated
//!     raise delivers. Resolve the gating finding and the next raise comes back clean.
//!
//! Every category value in this file is seeded BY THIS TEST as workspace data. That is the point:
//! grep `rust/crates/` for one of them and you will find it only here, never in the crate.
//!
//! Real booted `Node`: real store (`mem://`), real tag graph, real caps, the real `call_tool`
//! bridge, and real channel deliveries read back through the real inbox. No mocks (CLAUDE §9).

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, Node};
use lb_mcp::ToolError;
use serde_json::{json, Value};

const RAISE: &str = "mcp:insight.raise:call";
const GET: &str = "mcp:insight.get:call";
const LIST: &str = "mcp:insight.list:call";
const RESOLVE: &str = "mcp:insight.resolve:call";
const SUB_CREATE: &str = "mcp:insight.sub.create:call";
const CHAN_PUB: &str = "bus:chan/*:pub";
const INBOX_LIST: &str = "mcp:inbox.list:call";

/// The workspace's declared vocabulary, seeded by this test the way a pack would seed it. These
/// strings are DATA — they exist in this file and nowhere in the crate.
const VALUES: [&str; 3] = ["dq", "device", "optimisation"];
/// The value this workspace declares as gating. Named `dq` rather than anything product-shaped so
/// no reader mistakes it for a vocabulary lb knows.
const GATE: &str = "dq";

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

fn caps() -> Vec<&'static str> {
    vec![
        RAISE, GET, LIST, RESOLVE, SUB_CREATE, CHAN_PUB, INBOX_LIST,
    ]
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

/// Seed the `tag_vocab:category` row a pack would install. Written straight to the store because
/// the vocabulary is workspace CONFIG, not something the insight surface mints.
async fn seed_vocab(node: &Arc<Node>, ws: &str) {
    lb_store::write(
        &node.store,
        ws,
        "tag_vocab",
        "category",
        &json!({ "key": "category", "values": VALUES, "gates": [GATE] }),
    )
    .await
    .expect("vocab seeded");
}

/// A raise carrying an optional category and optional evidence subjects.
fn raise_input(dedup_key: &str, severity: &str, ts: u64, category: Option<&str>, subjects: &[&str]) -> Value {
    let mut input = json!({
        "dedup_key": dedup_key,
        "severity": severity,
        "title": format!("finding {dedup_key}"),
        "origin": { "kind": "rule", "ref": "rule:r1" },
        "ts": ts,
    });
    if let Some(c) = category {
        input["tags"] = json!({ "category": c });
    }
    if !subjects.is_empty() {
        input["evidence"] = json!({ "source": "demo", "subjects": subjects });
    }
    input
}

async fn raise(node: &Arc<Node>, p: &Principal, ws: &str, input: Value) -> Result<Value, ToolError> {
    call(node, p, ws, "insight.raise", input).await
}

// --- (f) the workspace-declared category vocabulary ----------------------------------------------

/// **lb ships no default value list.** A workspace nobody seeded accepts anything, exactly as it did
/// before this validation existed — so adding it breaks no existing workspace and encodes no
/// product taxonomy in lb (rule 10).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_unseeded_workspace_validates_nothing() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    let p = principal("user:test", ws, &caps());

    let out = raise(
        &node,
        &p,
        ws,
        raise_input("k1", "warning", 1, Some("anything-at-all"), &[]),
    )
    .await;
    assert!(out.is_ok(), "an unseeded vocabulary is OPEN: {out:?}");
}

/// A seeded workspace rejects a value outside its declared set — **and names the set**, so a rule
/// author sees what they may have meant instead of a bare rejection. And the whole raise is
/// rejected: no record lands with a value nothing can group by.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_seeded_workspace_rejects_an_undeclared_value_and_names_the_set() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_vocab(&node, ws).await;
    let p = principal("user:test", ws, &caps());

    let err = raise(&node, &p, ws, raise_input("k1", "warning", 1, Some("nonsense"), &[]))
        .await
        .expect_err("outside the declared set");
    let msg = format!("{err:?}");
    assert!(msg.contains("nonsense"), "names the offending value: {msg}");
    for v in VALUES {
        assert!(msg.contains(v), "names the declared set ({v} missing): {msg}");
    }

    // Nothing was written — the guard runs before any store write.
    let page = call(&node, &p, ws, "insight.list", json!({}))
        .await
        .expect("list ok");
    assert!(
        page["items"].as_array().expect("items").is_empty(),
        "a rejected category leaves no partial record: {page}"
    );

    // A declared value goes straight through.
    assert!(raise(&node, &p, ws, raise_input("k1", "warning", 1, Some(VALUES[1]), &[]))
        .await
        .is_ok());
}

// --- (e) the caveat stamp ------------------------------------------------------------------------

/// **THE STAMP.** An open finding in the workspace's declared gating category, on `point:X`. A
/// second finding derived from `point:X` must carry its id — on the outcome, on `get`, and on the
/// roster page (`list`), because greying a soft row is a roster job.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_open_gating_finding_caveats_a_finding_on_the_same_subjects() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_vocab(&node, ws).await;
    let p = principal("user:test", ws, &caps());

    let dq = raise(
        &node,
        &p,
        ws,
        raise_input("sensor-stuck", "warning", 1, Some(GATE), &["point:X"]),
    )
    .await
    .expect("dq raise")["id"]
        .as_str()
        .unwrap()
        .to_string();

    let out = raise(
        &node,
        &p,
        ws,
        raise_input("intensity-high", "critical", 2, Some(VALUES[2]), &["point:X"]),
    )
    .await
    .expect("finding raise");
    assert_eq!(out["caveated"], true, "the outcome says so: {out}");

    let id = out["id"].as_str().unwrap();
    let got = call(&node, &p, ws, "insight.get", json!({ "id": id }))
        .await
        .expect("get ok");
    assert_eq!(got["caveats"], json!([dq]), "get echoes the caveat: {got}");

    let page = call(&node, &p, ws, "insight.list", json!({}))
        .await
        .expect("list ok");
    let row = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["id"] == id)
        .expect("row present");
    assert_eq!(row["caveats"], json!([dq]), "list echoes it too: {row}");

    // The gating finding is NOT caveated by itself, nor by another gating finding — a data-quality
    // finding is not softened by data quality, and mutual caveating would silence both.
    let dq_row = call(&node, &p, ws, "insight.get", json!({ "id": &dq }))
        .await
        .expect("get dq");
    assert!(
        dq_row["caveats"].as_array().map(|a| a.is_empty()).unwrap_or(true),
        "the gating finding stays uncaveated: {dq_row}"
    );
}

/// No subject overlap ⇒ no caveat. The join is on `evidence.subjects`, not on "there is a data
/// quality problem somewhere in this workspace".
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_finding_on_different_subjects_is_not_caveated() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_vocab(&node, ws).await;
    let p = principal("user:test", ws, &caps());

    raise(&node, &p, ws, raise_input("sensor-stuck", "warning", 1, Some(GATE), &["point:X"]))
        .await
        .expect("dq raise");
    let out = raise(
        &node,
        &p,
        ws,
        raise_input("other", "critical", 2, Some(VALUES[2]), &["point:Y"]),
    )
    .await
    .expect("raise");
    assert_eq!(out["caveated"], false, "different points, no caveat: {out}");
}

/// **Self-healing.** Resolve the gating finding and the next raise of the dependent one comes back
/// clean. The caveat is a statement about the world right now, so it is REFRESHED (not merged) on
/// every raise — a permanent caveat is the failure mode that teaches operators to ignore it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn resolving_the_gating_finding_clears_the_caveat_on_the_next_raise() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_vocab(&node, ws).await;
    let p = principal("user:test", ws, &caps());

    let dq = raise(&node, &p, ws, raise_input("sensor-stuck", "warning", 1, Some(GATE), &["point:X"]))
        .await
        .expect("dq raise")["id"]
        .as_str()
        .unwrap()
        .to_string();
    let id = raise(&node, &p, ws, raise_input("dependent", "critical", 2, Some(VALUES[2]), &["point:X"]))
        .await
        .expect("raise")["id"]
        .as_str()
        .unwrap()
        .to_string();

    call(&node, &p, ws, "insight.resolve", json!({ "id": &dq, "ts": 3 }))
        .await
        .expect("resolved");

    let out = raise(&node, &p, ws, raise_input("dependent", "critical", 4, Some(VALUES[2]), &["point:X"]))
        .await
        .expect("re-raise");
    assert_eq!(out["caveated"], false, "the caveat cleared: {out}");
    let got = call(&node, &p, ws, "insight.get", json!({ "id": &id }))
        .await
        .expect("get ok");
    assert!(
        got["caveats"].as_array().map(|a| a.is_empty()).unwrap_or(true),
        "the stored list was REFRESHED, not merged: {got}"
    );
}

/// **THE DELIVERY ASSERTION.** A `critical` first-ever raise is the loudest thing the notify path
/// can produce. Caveated, it must post NOTHING; the identical uncaveated raise on a sibling key
/// must post. Asserted through a real subscription and the real delivered inbox — the control half
/// is what stops this passing because the machine happened to be silent.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_caveated_critical_raise_delivers_nothing_while_an_uncaveated_one_does() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_vocab(&node, ws).await;
    let p = principal("user:test", ws, &caps());

    // One subscription over everything, into a real channel.
    call(
        &node,
        &p,
        ws,
        "insight.sub.create",
        json!({ "sink": { "kind": "channel", "channel": "ops" }, "filter": {}, "now": 1 }),
    )
    .await
    .expect("sub created");

    raise(&node, &p, ws, raise_input("sensor-stuck", "warning", 10, Some(GATE), &["point:X"]))
        .await
        .expect("dq raise");

    // Caveated (subjects overlap the open gating finding) …
    raise(&node, &p, ws, raise_input("caveated-key", "critical", 20, Some(VALUES[2]), &["point:X"]))
        .await
        .expect("caveated raise");
    // … and the control, identical but for its subjects.
    raise(&node, &p, ws, raise_input("clean-key", "critical", 30, Some(VALUES[2]), &["point:Y"]))
        .await
        .expect("clean raise");

    let items = lb_host::list_inbox(&node.store, &p, ws, "ops")
        .await
        .expect("inbox readable");
    let posts = |key: &str| items.iter().filter(|i| i.body.contains(key)).count();

    assert_eq!(
        posts("clean-key"),
        1,
        "the CONTROL must deliver, or this test proves nothing: {:?}",
        items.iter().map(|i| &i.body).collect::<Vec<_>>()
    );
    assert_eq!(
        posts("caveated-key"),
        0,
        "a caveated critical raise must not break through and must not post immediately: {:?}",
        items.iter().map(|i| &i.body).collect::<Vec<_>>()
    );
}

// --- MANDATORY: workspace isolation --------------------------------------------------------------

/// A gating finding in `ws-a` must not caveat a finding in `ws-b`, even on the identical subject.
/// The vocabulary AND the scan are both ws-scoped; a cross-ws caveat would leak the existence of
/// another workspace's finding through a field on this one's record.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_gating_finding_does_not_caveat_across_workspaces() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_vocab(&node, "ws-a").await;
    seed_vocab(&node, "ws-b").await;
    let a = principal("user:test", "ws-a", &caps());
    let b = principal("user:test", "ws-b", &caps());

    raise(&node, &a, "ws-a", raise_input("sensor-stuck", "warning", 1, Some(GATE), &["point:X"]))
        .await
        .expect("dq raise in ws-a");

    let out = raise(
        &node,
        &b,
        "ws-b",
        raise_input("dependent", "critical", 2, Some(VALUES[2]), &["point:X"]),
    )
    .await
    .expect("raise in ws-b");
    assert_eq!(out["caveated"], false, "no cross-workspace caveat: {out}");
}
