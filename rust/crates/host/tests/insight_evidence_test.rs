//! `evidence` — the finding's statement of the data that proves it, over a REAL booted `Node`
//! (`docs/scope/insights/insight-evidence-scope.md`). Real store, real bus, real caps, the real
//! `call_tool` MCP bridge. NO mocks (CLAUDE §9): records are seeded by raising through the verb
//! under test and read back through it.
//!
//! Mandatory categories: capability-deny (evidence must not open an alternate read path) +
//! workspace-isolation. Scope-named cases: round-trip, the migration guard, dedup-refresh (the
//! deliberate divergence from `body`/`title`), the 4 KB reject, the get-vs-list boundary, and the
//! bare-string `series` sugar.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, Node};
use lb_mcp::ToolError;
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

/// The evidence fixture, modelled on the case that motivated the scope: a BUILDING-level finding
/// whose judgment query is a `GROUP BY` aggregate (unplottable) and whose `series` is therefore a
/// different, per-entity query. Domain-free identity lives in the dedup_key, never the title.
fn evidence() -> Value {
    json!({
        "source": "demo-buildings",
        "series": [{
            "sql": "SELECT time, value FROM point_reading WHERE point_id = 'p-1' ORDER BY time",
            "label": "Energy kWh",
            "unit": "kWh",
        }],
        "query": "SELECT s.name, SUM(pr.value) FROM point_reading pr GROUP BY s.name",
        "window": { "from": 1_700_000_000_000u64, "to": 1_700_086_400_000u64 },
        "threshold": 1.0,
    })
}

fn raise_input(dedup_key: &str, ts: u64, ev: Option<Value>) -> Value {
    let mut v = json!({
        "dedup_key": dedup_key,
        "severity": "warning",
        "title": "intensity above budget",
        "body": { "kwh_per_m2": 2.4, "budget": 1.0 },
        "origin": { "kind": "rule", "ref": "rule:intensity" },
        "ts": ts,
    });
    if let Some(ev) = ev {
        v["evidence"] = ev;
    }
    v
}

// --- round-trip: what the producer stated is what `insight.get` echoes ---------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn get_echoes_the_evidence_the_producer_stated() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:ada", "acme", &[RAISE, GET]);
    let out = call(
        &node,
        &p,
        "acme",
        "insight.raise",
        raise_input("k1", 1, Some(evidence())),
    )
    .await
    .expect("raise ok");
    let id = out["id"].as_str().unwrap();

    let got = call(&node, &p, "acme", "insight.get", json!({ "id": id }))
        .await
        .expect("get ok");
    assert_eq!(
        got["evidence"],
        evidence(),
        "evidence round-trips byte-identically"
    );
}

// --- THE MIGRATION GUARD: the regression that would silently empty every roster -------------
// `insight.list` decodes with `filter_map(|v| from_value(v).ok())` — a record that fails to decode
// is DROPPED with no error anywhere. `evidence` is `Option` + `#[serde(default)]` +
// `skip_serializing_if`, so a raise that states none stores a blob with NO `evidence` key — byte
// -identical to every record written before the field existed. If the field were ever made
// required, this test is the one that catches it, and it catches it as "the list is empty" rather
// than as a decode error, which is exactly why it is worth an explicit case.

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_record_with_no_evidence_key_still_lists_and_gets() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:ada", "acme", &[RAISE, GET, LIST]);
    let out = call(
        &node,
        &p,
        "acme",
        "insight.raise",
        raise_input("legacy", 1, None),
    )
    .await
    .expect("raise ok");
    let id = out["id"].as_str().unwrap();

    let page = call(&node, &p, "acme", "insight.list", json!({}))
        .await
        .expect("list ok");
    let items = page["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "a pre-field-shaped record still lists");
    assert_eq!(items[0]["dedup_key"], "legacy");

    let got = call(&node, &p, "acme", "insight.get", json!({ "id": id }))
        .await
        .expect("get ok");
    assert!(
        got.get("evidence").is_none(),
        "absent evidence serializes to nothing, not null: {got}"
    );
}

// --- dedup: evidence REFRESHES, unlike title/body ------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn re_raise_refreshes_evidence_but_leaves_title_and_body_first_raise_wins() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:ada", "acme", &[RAISE, GET]);

    let first = call(
        &node,
        &p,
        "acme",
        "insight.raise",
        raise_input("k", 1, Some(evidence())),
    )
    .await
    .expect("raise 1");
    let id = first["id"].as_str().unwrap().to_string();

    // Second raise: same key, DIFFERENT evidence and a different title/body.
    let mut second = raise_input(
        "k",
        2,
        Some(json!({
            "source": "other-source",
            "series": [{ "sql": "SELECT time, value FROM t2 ORDER BY time" }],
        })),
    );
    second["title"] = json!("a different title");
    second["body"] = json!({ "kwh_per_m2": 9.9 });
    call(&node, &p, "acme", "insight.raise", second)
        .await
        .expect("raise 2");

    let got = call(&node, &p, "acme", "insight.get", json!({ "id": &id }))
        .await
        .expect("get ok");
    assert_eq!(got["count"], 2, "dedup bumped, not duplicated");
    // Evidence is a BINDING — the latest wins, so a rule edited to query a renamed table heals.
    assert_eq!(
        got["evidence"]["source"], "other-source",
        "evidence refreshed"
    );
    assert!(
        got["evidence"].get("threshold").is_none(),
        "wholly replaced, not merged"
    );
    // …while title/body stay first-raise-wins. Asserted here so the DIVERGENCE is documented in a
    // test rather than only in a comment (see the scope's open question 1).
    assert_eq!(
        got["title"], "intensity above budget",
        "title first-raise-wins"
    );
    assert_eq!(got["body"]["kwh_per_m2"], 2.4, "body first-raise-wins");

    // Third raise OMITTING evidence must not blank the stored binding.
    call(
        &node,
        &p,
        "acme",
        "insight.raise",
        raise_input("k", 3, None),
    )
    .await
    .expect("raise 3");
    let got = call(&node, &p, "acme", "insight.get", json!({ "id": &id }))
        .await
        .expect("get ok");
    assert_eq!(
        got["evidence"]["source"], "other-source",
        "a raise with no evidence leaves the stored binding alone"
    );
}

// --- the get-vs-list boundary ---------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn list_omits_evidence_while_get_echoes_it() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:ada", "acme", &[RAISE, GET, LIST]);
    let out = call(
        &node,
        &p,
        "acme",
        "insight.raise",
        raise_input("k", 1, Some(evidence())),
    )
    .await
    .expect("raise ok");
    let id = out["id"].as_str().unwrap();

    let page = call(&node, &p, "acme", "insight.list", json!({}))
        .await
        .expect("list ok");
    let item = &page["items"].as_array().unwrap()[0];
    assert!(
        item.get("evidence").is_none(),
        "list omits the descriptor (page bloat + schema disclosure): {item}"
    );
    assert_eq!(item["dedup_key"], "k", "the rest of the record is intact");

    let got = call(&node, &p, "acme", "insight.get", json!({ "id": id }))
        .await
        .expect("get ok");
    assert_eq!(got["evidence"]["source"], "demo-buildings", "get echoes it");
}

// --- capability-deny: evidence opens no alternate path ---------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn raise_with_evidence_is_denied_without_the_raise_cap() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    // Holds every READ cap but not raise.
    let p = principal("user:mallory", "acme", &[GET, LIST]);
    let r = call(
        &node,
        &p,
        "acme",
        "insight.raise",
        raise_input("k", 1, Some(evidence())),
    )
    .await;
    assert!(
        matches!(r, Err(ToolError::Denied) | Err(ToolError::NotFound)),
        "denied opaquely, exactly as without evidence"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_lister_without_get_never_receives_an_evidence_payload() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let author = principal("user:ada", "acme", &[RAISE]);
    call(
        &node,
        &author,
        "acme",
        "insight.raise",
        raise_input("k", 1, Some(evidence())),
    )
    .await
    .expect("raise ok");

    // A principal holding LIST but not GET has no path to the descriptor at all.
    let reader = principal("user:bob", "acme", &[LIST]);
    let page = call(&node, &reader, "acme", "insight.list", json!({}))
        .await
        .expect("list ok");
    let dump = page.to_string();
    assert!(
        !dump.contains("demo-buildings") && !dump.contains("point_reading"),
        "no datasource id and no SQL reaches a list-only reader: {dump}"
    );
}

// --- workspace isolation ----------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn evidence_never_leaks_across_workspaces() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let a = principal("user:ada", "ws-a", &[RAISE, GET, LIST]);
    let out = call(
        &node,
        &a,
        "ws-a",
        "insight.raise",
        raise_input("k", 1, Some(evidence())),
    )
    .await
    .expect("raise ok");
    let id = out["id"].as_str().unwrap();

    let b = principal("user:bob", "ws-b", &[RAISE, GET, LIST]);
    let got = call(&node, &b, "ws-b", "insight.get", json!({ "id": id })).await;
    let leaked = got.map(|v| v.to_string()).unwrap_or_default();
    assert!(
        !leaked.contains("demo-buildings"),
        "ws-b cannot read ws-a's evidence: {leaked}"
    );
    let page = call(&node, &b, "ws-b", "insight.list", json!({}))
        .await
        .expect("list ok");
    assert_eq!(
        page["items"].as_array().unwrap().len(),
        0,
        "no cross-ws rows"
    );
}

// --- the 4 KB reject ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn oversize_evidence_rejects_the_whole_raise() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:ada", "ws-a", &[RAISE, LIST]);
    let big = "x".repeat(5000);
    let r = call(
        &node,
        &p,
        "ws-a",
        "insight.raise",
        raise_input(
            "k",
            1,
            Some(json!({ "source": "s", "series": [{ "sql": big }] })),
        ),
    )
    .await;
    assert!(
        matches!(r, Err(ToolError::BadInput(_))),
        "oversize evidence rejects"
    );
    // Rejected UP FRONT — no orphan parent row, exactly like the occurrence cap.
    let page = call(&node, &p, "ws-a", "insight.list", json!({}))
        .await
        .expect("list ok");
    assert_eq!(page["items"].as_array().unwrap().len(), 0, "no orphan row");
}

// --- authoring sugar: a bare string is a series ------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_bare_string_series_decodes_to_a_full_series_object() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:ada", "acme", &[RAISE, GET]);
    let out = call(
        &node,
        &p,
        "acme",
        "insight.raise",
        raise_input(
            "k",
            1,
            Some(json!({ "source": "s", "series": ["SELECT time, value FROM t"] })),
        ),
    )
    .await
    .expect("raise ok");
    let id = out["id"].as_str().unwrap();

    let got = call(&node, &p, "acme", "insight.get", json!({ "id": id }))
        .await
        .expect("get ok");
    // One stored shape regardless of which authoring shape was used.
    assert_eq!(
        got["evidence"]["series"][0]["sql"],
        "SELECT time, value FROM t"
    );
    assert!(got["evidence"]["series"][0].get("label").is_none());
}

// --- a partial descriptor is legal (source-only) -------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn evidence_with_only_a_source_is_accepted() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:ada", "acme", &[RAISE, GET]);
    let out = call(
        &node,
        &p,
        "acme",
        "insight.raise",
        raise_input("k", 1, Some(json!({ "source": "s" }))),
    )
    .await
    .expect("raise ok");
    let id = out["id"].as_str().unwrap();
    let got = call(&node, &p, "acme", "insight.get", json!({ "id": id }))
        .await
        .expect("get ok");
    assert_eq!(got["evidence"]["source"], "s");
    assert!(
        got["evidence"].get("series").is_none(),
        "an empty series list serializes to nothing"
    );
}
