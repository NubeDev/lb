//! The **tag echo** — the insight's dimension facets, materialized from the tag GRAPH onto the
//! record and returned by BOTH `insight.get` and `insight.list`
//! (`docs/scope/insights/insight-tag-echo-scope.md`).
//!
//! Real booted `Node`: real store (`mem://`), real tag graph, real caps, the real `call_tool` MCP
//! bridge. NO mocks (CLAUDE §9) — every record is seeded by raising through the verb under test and
//! read back through it, and every out-of-band tag is applied through the real `tags.add` verb.
//!
//! Mandatory categories: capability-deny (a real id and a fictional id produce the IDENTICAL error;
//! and the echo needs no tag caps at all) + workspace-isolation (ws-A and ws-B carrying the SAME
//! tag key and value — a denormalized copy of graph data is exactly where cross-ws bleed hides).
//!
//! Three of these are REVERT-CHECKED (the property only the fix satisfies, per the scope):
//!   - [`echo_is_the_union_across_raises_not_this_raises_declaration`] — write the echo from
//!     `RaiseInput.tags` instead of the graph ⇒ red.
//!   - [`list_filtering_reads_the_graph_not_the_stale_echo`] — resolve the filter off the echo ⇒ red.
//!   - [`echo_is_written_with_zero_subscriptions_in_the_workspace`] — restore the
//!     `if !subs.is_empty()` guard around materialization ⇒ red.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tags_tool, call_tool, Node};
use lb_mcp::ToolError;
use serde_json::{json, Value};

const RAISE: &str = "mcp:insight.raise:call";
const GET: &str = "mcp:insight.get:call";
const LIST: &str = "mcp:insight.list:call";
const SUB_LIST: &str = "mcp:insight.sub.list:call";
const TAGS_ADD: &str = "mcp:tags.add:call";
const TAGS_OF: &str = "mcp:tags.of:call";

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

/// A raise carrying `tags` as its declared dimension map. Domain-free identity stays in the
/// dedup_key (the tag plane's cardinality rule), dimensions in the tags.
fn raise_input(dedup_key: &str, ts: u64, tags: Value) -> Value {
    json!({
        "dedup_key": dedup_key,
        "severity": "warning",
        "title": "intensity above budget",
        "origin": { "kind": "rule", "ref": "rule:intensity" },
        "tags": tags,
        "ts": ts,
    })
}

/// Raise and return the minted insight id.
async fn raise_id(node: &Arc<Node>, p: &Principal, ws: &str, input: Value) -> String {
    let out = call(node, p, ws, "insight.raise", input)
        .await
        .expect("raise ok");
    out["id"].as_str().expect("id").to_string()
}

/// The single roster row for `id` out of a `list` page (the assertion surface that matters — a test
/// that only checks `get` passes while the roster stays broken).
async fn list_row(node: &Arc<Node>, p: &Principal, ws: &str, id: &str, filter: Value) -> Value {
    let page = call(node, p, ws, "insight.list", filter)
        .await
        .expect("list ok");
    page["items"]
        .as_array()
        .expect("items")
        .iter()
        .find(|i| i["id"] == id)
        .cloned()
        .unwrap_or(Value::Null)
}

// --- case 1: the echo lands on raise and rides the LIST page ---------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn echo_lands_on_raise_and_appears_in_both_get_and_list() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:ada", "acme", &[RAISE, GET, LIST]);
    let id = raise_id(
        &node,
        &p,
        "acme",
        raise_input(
            "k",
            1,
            json!({ "building": "chullora-dc", "asset_type": "water-meter", "priority": "medium" }),
        ),
    )
    .await;

    let got = call(&node, &p, "acme", "insight.get", json!({ "id": &id }))
        .await
        .expect("get ok");
    assert_eq!(
        got["tags"],
        json!({ "building": "chullora-dc", "asset_type": "water-meter", "priority": "medium" }),
        "get echoes the facets"
    );

    // THE POINT OF THE SCOPE: the same map on the roster page, from ONE call and no `tags.find`.
    let row = list_row(&node, &p, "acme", &id, json!({})).await;
    assert_eq!(
        row["tags"],
        json!({ "building": "chullora-dc", "asset_type": "water-meter", "priority": "medium" }),
        "list rows carry the dimension columns: {row}"
    );
}

// --- case 2 (REVERT-CHECKED): the union across raises, not this raise's declaration -----------
// The bug this scope exists to avoid, and the one a future refactor will reintroduce ("just store
// input.tags, we already have it"): a producer that stops sending `classification` would blank the
// column for every row it owns.

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn echo_is_the_union_across_raises_not_this_raises_declaration() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:ada", "acme", &[RAISE, GET, LIST]);

    let id = raise_id(
        &node,
        &p,
        "acme",
        raise_input("k", 1, json!({ "building": "chullora-dc" })),
    )
    .await;
    // Same dedup_key, a DISJOINT declaration — `building` is not restated.
    let again = raise_id(
        &node,
        &p,
        "acme",
        raise_input("k", 2, json!({ "asset_type": "water-meter" })),
    )
    .await;
    assert_eq!(again, id, "dedup bumped the same record");

    let row = list_row(&node, &p, "acme", &id, json!({})).await;
    assert_eq!(
        row["tags"],
        json!({ "building": "chullora-dc", "asset_type": "water-meter" }),
        "the echo is the UNION across raises, read from the graph: {row}"
    );
}

// --- case 3: self-heal after an out-of-band tag change ----------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn echo_self_heals_on_the_next_raise_after_an_out_of_band_tag() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:ada", "acme", &[RAISE, GET, LIST, TAGS_ADD]);
    let id = raise_id(
        &node,
        &p,
        "acme",
        raise_input("k", 1, json!({ "building": "chullora-dc" })),
    )
    .await;

    // An admin classifies the finding through the EXISTING tags verb — the graph moves, the echo
    // does not (the accepted divergence window).
    // Through the REAL `tags.add` verb + its own cap gate. (`tags.*` is reached via the tags MCP
    // bridge, not the `call_tool` host-native table.)
    call_tags_tool(
        &node.store,
        &p,
        "acme",
        "tags.add",
        &json!({ "entity": format!("insight:{id}"), "key": "classification", "value": "mechanical" }),
    )
    .await
    .expect("tags.add ok");
    let stale = call(&node, &p, "acme", "insight.get", json!({ "id": &id }))
        .await
        .expect("get ok");
    assert!(
        stale["tags"].get("classification").is_none(),
        "the echo is briefly stale — it is written by the raise path only: {stale}"
    );

    // The rule fires again that night. No special path, no repair verb.
    raise_id(
        &node,
        &p,
        "acme",
        raise_input("k", 2, json!({ "building": "chullora-dc" })),
    )
    .await;
    let healed = call(&node, &p, "acme", "insight.get", json!({ "id": &id }))
        .await
        .expect("get ok");
    assert_eq!(
        healed["tags"],
        json!({ "building": "chullora-dc", "classification": "mechanical" }),
        "the echo converged on the graph: {healed}"
    );
}

// --- case 4 (REVERT-CHECKED): filtering reads the GRAPH, never the echo -----------------------
// If someone "simplifies" `insight.list`'s facet filter to scan the echo, this goes red: the record
// matches a tag it demonstrably does not carry in its own projection yet.

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn list_filtering_reads_the_graph_not_the_stale_echo() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:ada", "acme", &[RAISE, GET, LIST, TAGS_ADD]);
    let id = raise_id(
        &node,
        &p,
        "acme",
        raise_input("k", 1, json!({ "building": "chullora-dc" })),
    )
    .await;

    // Out-of-band tag, and deliberately NO re-raise: the graph knows, the echo does not.
    // Through the REAL `tags.add` verb + its own cap gate. (`tags.*` is reached via the tags MCP
    // bridge, not the `call_tool` host-native table.)
    call_tags_tool(
        &node.store,
        &p,
        "acme",
        "tags.add",
        &json!({ "entity": format!("insight:{id}"), "key": "classification", "value": "mechanical" }),
    )
    .await
    .expect("tags.add ok");

    let row = list_row(
        &node,
        &p,
        "acme",
        &id,
        json!({ "tags": { "classification": "mechanical" } }),
    )
    .await;
    assert_eq!(
        row["id"], id,
        "the graph is the truth for FILTERING — the row matches a facet its echo hasn't caught up to"
    );
    assert!(
        row["tags"].get("classification").is_none(),
        "…and it is genuinely stale, so this is not a vacuous pass: {row}"
    );
}

// --- case 5: the echo is host-computed, never caller-writable ---------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_echo_is_not_caller_writable() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:ada", "acme", &[RAISE, GET, LIST]);
    let id = raise_id(
        &node,
        &p,
        "acme",
        raise_input("k", 1, json!({ "building": "chullora-dc" })),
    )
    .await;

    // A re-raise declaring NO tags cannot blank the projection, and a record-level `tags` value
    // smuggled alongside is ignored — the echo comes from the graph, like `producer` comes from the
    // principal.
    let mut sneaky = raise_input("k", 2, json!({}));
    sneaky["tags"] = json!({});
    sneaky["producer"] = json!("user:someone-else");
    call(&node, &p, "acme", "insight.raise", sneaky)
        .await
        .expect("raise ok");

    let got = call(&node, &p, "acme", "insight.get", json!({ "id": &id }))
        .await
        .expect("get ok");
    assert_eq!(
        got["tags"],
        json!({ "building": "chullora-dc" }),
        "the echo tracks the graph, not the caller: {got}"
    );
    assert_eq!(
        got["producer"], "user:ada",
        "the host-stamp precedent this follows still holds"
    );
}

// --- case 6 (REVERT-CHECKED): materialization is UNCONDITIONAL -------------------------------
// Materializing only `if !subs.is_empty()` would leave the dimension columns blank in every
// workspace that notifies nobody — i.e. the common case.

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn echo_is_written_with_zero_subscriptions_in_the_workspace() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:ada", "acme", &[RAISE, GET, LIST, SUB_LIST]);

    // Pin the precondition — otherwise this test silently becomes case 1 the day a fixture seeds a
    // subscription somewhere.
    let subs = call(&node, &p, "acme", "insight.sub.list", json!({}))
        .await
        .expect("sub.list ok");
    assert_eq!(
        subs["subs"].as_array().map(|a| a.len()),
        Some(0),
        "precondition: no subscriptions in this workspace: {subs}"
    );

    let id = raise_id(
        &node,
        &p,
        "acme",
        raise_input("k", 1, json!({ "building": "chullora-dc" })),
    )
    .await;
    let row = list_row(&node, &p, "acme", &id, json!({})).await;
    assert_eq!(
        row["tags"],
        json!({ "building": "chullora-dc" }),
        "the echo does not depend on anyone subscribing: {row}"
    );
}

// --- capability deny (mandatory) ---------------------------------------------------------------

/// The OUTER gate's own property: without `mcp:insight.get:call`, a REAL id and a FICTIONAL id
/// produce the IDENTICAL error — the denial cannot be used as an existence oracle. An inner layer
/// (the store read) would distinguish them, so this is the assertion the gate alone satisfies.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_denied_get_cannot_distinguish_a_real_id_from_a_fictional_one() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let author = principal("user:ada", "acme", &[RAISE]);
    let id = raise_id(
        &node,
        &author,
        "acme",
        raise_input("k", 1, json!({ "building": "chullora-dc" })),
    )
    .await;

    let mallory = principal("user:mallory", "acme", &[LIST]); // every read cap EXCEPT get
    let real = call(&node, &mallory, "acme", "insight.get", json!({ "id": &id })).await;
    let fake = call(
        &node,
        &mallory,
        "acme",
        "insight.get",
        json!({ "id": "01NOSUCHINSIGHT0000000000" }),
    )
    .await;
    let (real_err, fake_err) = (format!("{real:?}"), format!("{fake:?}"));
    assert!(matches!(real, Err(ToolError::Denied)), "denied: {real_err}");
    assert_eq!(
        real_err, fake_err,
        "a real id and a fictional one are indistinguishable through the gate"
    );
    assert!(
        !real_err.contains("chullora"),
        "no facet data in the error: {real_err}"
    );

    // …and the difference IS observable to a holder of the cap — so the assertion above is the
    // gate's doing, not an accident of both ids being unreadable.
    let ada = principal("user:ada", "acme", &[GET]);
    let ok_real = call(&node, &ada, "acme", "insight.get", json!({ "id": &id }))
        .await
        .expect("get ok");
    let ok_fake = call(
        &node,
        &ada,
        "acme",
        "insight.get",
        json!({ "id": "01NOSUCHINSIGHT0000000000" }),
    )
    .await
    .expect("get ok");
    assert_eq!(ok_real["tags"]["building"], "chullora-dc");
    assert!(ok_fake.is_null(), "fictional id reads as absent: {ok_fake}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_denied_list_leaks_no_facet_data() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let author = principal("user:ada", "acme", &[RAISE]);
    raise_id(
        &node,
        &author,
        "acme",
        raise_input("k", 1, json!({ "building": "chullora-dc" })),
    )
    .await;

    let mallory = principal("user:mallory", "acme", &[GET]); // holds get, NOT list
    let denied = call(&node, &mallory, "acme", "insight.list", json!({})).await;
    let dump = format!("{denied:?}");
    assert!(matches!(denied, Err(ToolError::Denied)), "denied: {dump}");
    assert!(!dump.contains("chullora"), "no facets in the error: {dump}");
}

/// The NARROWING this scope buys: dimension columns used to need `mcp:tags.of:call` on top of the
/// insight read caps. A roster reader now holds `insight.list` and NOTHING else.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_lister_with_no_tag_caps_at_all_still_receives_the_echo() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let author = principal("user:ada", "acme", &[RAISE]);
    let id = raise_id(
        &node,
        &author,
        "acme",
        raise_input("k", 1, json!({ "building": "chullora-dc" })),
    )
    .await;

    let reader = principal("user:bob", "acme", &[LIST]);
    let row = list_row(&node, &reader, "acme", &id, json!({})).await;
    assert_eq!(
        row["tags"]["building"], "chullora-dc",
        "the echo rides the insight read cap alone: {row}"
    );
    // The tag graph itself stays shut to them — the echo is a projection, not a door.
    let denied = call(
        &node,
        &reader,
        "acme",
        "tags.of",
        json!({ "entity": format!("insight:{id}") }),
    )
    .await;
    assert!(
        matches!(denied, Err(ToolError::Denied)),
        "no tags cap was granted: {denied:?}"
    );
}

// --- workspace isolation (mandatory) -----------------------------------------------------------

/// ws-A and ws-B raise insights carrying the SAME tag key AND the same value — so a leak through
/// the shared tag vocabulary (a graph query that forgot its namespace, an echo copied across the
/// wall) shows up as a wrong row rather than an empty one.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_echo_never_crosses_the_workspace_wall() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let a = principal("user:ada", "ws-a", &[RAISE, GET, LIST, TAGS_OF]);
    let b = principal("user:bob", "ws-b", &[RAISE, GET, LIST, TAGS_OF]);

    let a_id = raise_id(
        &node,
        &a,
        "ws-a",
        raise_input(
            "shared-key",
            1,
            json!({ "building": "chullora-dc", "owner": "ws-a-only" }),
        ),
    )
    .await;
    let b_id = raise_id(
        &node,
        &b,
        "ws-b",
        raise_input(
            "shared-key",
            1,
            json!({ "building": "chullora-dc", "owner": "ws-b-only" }),
        ),
    )
    .await;
    assert_ne!(a_id, b_id, "distinct records behind the wall");

    // ws-B's roster: its own row, its own facets, and no trace of ws-A's.
    let page = call(&node, &b, "ws-b", "insight.list", json!({}))
        .await
        .expect("list ok");
    let items = page["items"].as_array().expect("items");
    assert_eq!(items.len(), 1, "one row: {page}");
    assert_eq!(items[0]["id"], b_id);
    assert_eq!(items[0]["tags"]["owner"], "ws-b-only");
    assert!(
        !page.to_string().contains("ws-a-only"),
        "no ws-A facet reaches ws-B: {page}"
    );

    // The shared facet VALUE resolves only within the caller's own workspace.
    let filtered = call(
        &node,
        &b,
        "ws-b",
        "insight.list",
        json!({ "tags": { "building": "chullora-dc" } }),
    )
    .await
    .expect("list ok");
    let ids: Vec<&str> = filtered["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|i| i["id"].as_str())
        .collect();
    assert_eq!(ids, vec![b_id.as_str()], "facet search is ws-walled");

    // A direct get across the wall reads as absent, with no facets anywhere in the reply.
    let cross = call(&node, &b, "ws-b", "insight.get", json!({ "id": &a_id }))
        .await
        .expect("get ok");
    assert!(cross.is_null(), "ws-A's record is invisible: {cross}");
}

// --- the cardinality guard --------------------------------------------------------------------

/// An absurd tag map must not produce an unbounded record. The echo is host-computed AFTER the
/// durable record landed, so the guard's contract is a LOUD SKIP (a warn + the previous echo left
/// in place), never a silent truncation and never a failed raise.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_oversize_facet_set_skips_the_echo_instead_of_bloating_the_record() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:ada", "acme", &[RAISE, GET, LIST]);

    // ~200 dimensions × ~40 bytes ⇒ well past the 2 KB echo cap.
    let mut tags = serde_json::Map::new();
    for i in 0..200 {
        tags.insert(format!("dimension-number-{i:03}"), json!("a-facet-value"));
    }
    let id = raise_id(&node, &p, "acme", raise_input("k", 1, Value::Object(tags))).await;

    let got = call(&node, &p, "acme", "insight.get", json!({ "id": &id }))
        .await
        .expect("the raise itself still succeeds");
    let echoed = got["tags"].as_object().map(|m| m.len()).unwrap_or(0);
    assert_eq!(
        echoed, 0,
        "over the cap the echo is skipped whole — never a silent partial map: {}",
        got["tags"]
    );
    assert_eq!(got["dedup_key"], "k", "the record itself is intact");
    assert!(
        serde_json::to_vec(&got).unwrap().len() < lb_insights::MAX_TAG_ECHO_BYTES,
        "the record did not grow unbounded"
    );
}
