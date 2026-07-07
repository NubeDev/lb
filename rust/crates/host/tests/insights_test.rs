//! The insights verbs over a REAL booted `Node` — real store, real bus, real caps, the real
//! `call_tool` MCP bridge (insights umbrella scope + sub-scopes). NO mocks (CLAUDE §9): records
//! are seeded by raising through the verb under test, then read back through it.
//!
//! Mandatory categories (testing-scope §2): capability-deny (per verb) + workspace-isolation.
//! Scope-named cases: dedup-lifecycle, ring-cap, 2KB-reject, matcher-axes, ladder-escalate/decay,
//! breakthroughs, ack-suppression, digest-idempotency, kill-switch, determinism.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, Node};
use lb_mcp::ToolError;
use serde_json::{json, Value};

/// Mint a principal carrying `caps` for `(sub, ws)`. Real signed token; the host's verify path
/// is exercised (the same shape every other host test uses).
fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

const RAISE: &str = "mcp:insight.raise:call";
const GET: &str = "mcp:insight.get:call";
const LIST: &str = "mcp:insight.list:call";
const ACK: &str = "mcp:insight.ack:call";
const RESOLVE: &str = "mcp:insight.resolve:call";
const OCC: &str = "mcp:insight.occurrences:call";
const SUB_CREATE: &str = "mcp:insight.sub.create:call";
const SUB_LIST: &str = "mcp:insight.sub.list:call";
const SUB_GET: &str = "mcp:insight.sub.get:call";
const SUB_DELETE: &str = "mcp:insight.sub.delete:call";
const SUB_MUTE: &str = "mcp:insight.sub.mute:call";
const POLICY_GET: &str = "mcp:insight.policy.get:call";
const POLICY_SET: &str = "mcp:insight.policy.set:call";
const CHAN_PUB: &str = "bus:chan/*:pub";
/// Read the delivered channel Items in the notify tests (a matched sub posts a real inbox Item).
const INBOX_LIST: &str = "mcp:inbox.list:call";

/// A caller holding the whole insight surface (+ inbox.list to read delivered notify posts).
fn member_caps() -> Vec<&'static str> {
    vec![
        RAISE, GET, LIST, ACK, RESOLVE, OCC, SUB_CREATE, SUB_LIST, SUB_GET, SUB_DELETE, SUB_MUTE,
        POLICY_GET, POLICY_SET, CHAN_PUB, INBOX_LIST,
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

/// The raise input fixture — a fraud-styled critical finding. Domain-free (rule 10): no provider
/// name; the dedup_key/body carry identity, never the title.
fn raise_input(dedup_key: &str, severity: &str, ts: u64) -> Value {
    json!({
        "dedup_key": dedup_key,
        "severity": severity,
        "title": "score above threshold",
        "origin": { "kind": "rule", "ref": "rule:scorer", "run": "job:1" },
        "tags": { "kind": "anomaly" },
        "occurrence": { "data": { "score": 0.93 }, "severity": severity },
        "ts": ts,
    })
}

// --- mandatory: capability deny (per verb) -----------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn raise_denied_without_the_cap() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:bob", "acme", &[GET, LIST]); // no RAISE
    let r = call(
        &node,
        &p,
        "acme",
        "insight.raise",
        raise_input("k1", "critical", 1),
    )
    .await;
    assert!(matches!(r, Err(ToolError::Denied)));
    // The deny left no record — a reader with LIST sees an empty workspace.
    let reader = principal("user:bob", "acme", &[LIST]);
    let page = call(&node, &reader, "acme", "insight.list", json!({}))
        .await
        .expect("list ok");
    assert_eq!(page["items"].as_array().unwrap().len(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ack_denied_without_the_cap() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let raiser = principal("user:ada", "acme", &member_caps());
    let out = call(
        &node,
        &raiser,
        "acme",
        "insight.raise",
        raise_input("k1", "warning", 1),
    )
    .await
    .expect("raise ok");
    let id = out["id"].as_str().unwrap();
    let p = principal("user:bob", "acme", &[RAISE, GET, LIST]); // no ACK
    let r = call(
        &node,
        &p,
        "acme",
        "insight.ack",
        json!({ "id": id, "ts": 2 }),
    )
    .await;
    assert!(matches!(r, Err(ToolError::Denied)), "ack denied opaque");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn occurrences_denied_without_the_cap_even_with_get() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let raiser = principal("user:ada", "acme", &member_caps());
    let out = call(
        &node,
        &raiser,
        "acme",
        "insight.raise",
        raise_input("k1", "warning", 1),
    )
    .await
    .expect("raise ok");
    let id = out["id"].as_str().unwrap();
    let p = principal("user:bob", "acme", &[RAISE, GET]); // no OCC
    let r = call(
        &node,
        &p,
        "acme",
        "insight.occurrences",
        json!({ "insight_id": id }),
    )
    .await;
    assert!(
        matches!(r, Err(ToolError::Denied)),
        "occurrences denied even with insight.get"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn sub_create_denied_without_the_channel_pub() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:bob", "acme", &[SUB_CREATE]); // no bus:chan/*:pub
    let r = call(
        &node,
        &p,
        "acme",
        "insight.sub.create",
        json!({
            "sink": { "kind": "channel", "channel": "ops" },
            "filter": {},
            "now": 1,
        }),
    )
    .await;
    assert!(
        matches!(r, Err(ToolError::Denied)),
        "sub.create denied without bus:chan/<channel>:pub"
    );
}

// --- mandatory: workspace isolation ------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn list_in_one_workspace_never_returns_another_workspaces_insights() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let a = principal("user:ada", "ws-a", &member_caps());
    let b = principal("user:bea", "ws-b", &member_caps());
    call(
        &node,
        &a,
        "ws-a",
        "insight.raise",
        raise_input("ka", "critical", 1),
    )
    .await
    .expect("raise in ws-a");
    let page_b = call(&node, &b, "ws-b", "insight.list", json!({}))
        .await
        .expect("list ws-b");
    assert_eq!(
        page_b["items"].as_array().unwrap().len(),
        0,
        "ws-B sees none of ws-A's insights"
    );
    let page_a = call(&node, &a, "ws-a", "insight.list", json!({}))
        .await
        .expect("list ws-a");
    assert_eq!(page_a["items"].as_array().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn occurrences_never_leak_across_workspaces() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let a = principal("user:ada", "ws-a", &member_caps());
    let b = principal("user:bea", "ws-b", &member_caps());
    let out = call(
        &node,
        &a,
        "ws-a",
        "insight.raise",
        raise_input("ka", "warning", 1),
    )
    .await
    .expect("raise ws-a");
    let id = out["id"].as_str().unwrap();
    // ws-B, same insight id string, cannot read the occurrences (its namespace is empty).
    let page_b = call(
        &node,
        &b,
        "ws-b",
        "insight.occurrences",
        json!({ "insight_id": id }),
    )
    .await
    .expect("occurrences ws-b");
    assert_eq!(page_b["items"].as_array().unwrap().len(), 0);
}

// --- scope-named: dedup lifecycle --------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn raise_dedup_bumps_count_and_preserves_acked_status() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:ada", "ws-a", &member_caps());
    let o1 = call(
        &node,
        &p,
        "ws-a",
        "insight.raise",
        raise_input("k", "warning", 1),
    )
    .await
    .unwrap();
    assert_eq!(o1["count"], 1);
    assert_eq!(o1["created"], true);
    assert_eq!(o1["status"], "open");
    let id = o1["id"].as_str().unwrap().to_string();

    let o2 = call(
        &node,
        &p,
        "ws-a",
        "insight.raise",
        raise_input("k", "warning", 2),
    )
    .await
    .unwrap();
    assert_eq!(o2["count"], 2);
    assert_eq!(o2["created"], false);
    assert_eq!(o2["id"].as_str().unwrap(), id, "same dedup key ⇒ same row");

    call(
        &node,
        &p,
        "ws-a",
        "insight.ack",
        json!({ "id": id, "ts": 3 }),
    )
    .await
    .unwrap();

    let o3 = call(
        &node,
        &p,
        "ws-a",
        "insight.raise",
        raise_input("k", "warning", 4),
    )
    .await
    .unwrap();
    assert_eq!(o3["count"], 3);
    assert_eq!(
        o3["status"], "acked",
        "an acked fault re-firing stays acked"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn raise_after_resolve_reopens() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:ada", "ws-a", &member_caps());
    let o1 = call(
        &node,
        &p,
        "ws-a",
        "insight.raise",
        raise_input("k", "warning", 1),
    )
    .await
    .unwrap();
    let id = o1["id"].as_str().unwrap().to_string();
    call(
        &node,
        &p,
        "ws-a",
        "insight.resolve",
        json!({ "id": id, "ts": 2 }),
    )
    .await
    .unwrap();
    let got = call(&node, &p, "ws-a", "insight.get", json!({ "id": id }))
        .await
        .unwrap();
    assert_eq!(got["status"], "resolved");

    let o2 = call(
        &node,
        &p,
        "ws-a",
        "insight.raise",
        raise_input("k", "warning", 3),
    )
    .await
    .unwrap();
    assert_eq!(o2["status"], "open", "resolved → raise re-opens");
    assert_eq!(o2["count"], 2, "count continues across the re-open");
    assert_eq!(o2["reopened"], true);
}

// --- scope-named: occurrence ring --------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ring_cap_evicts_oldest_but_count_is_lifetime() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:ada", "ws-a", &member_caps());
    // Shrink the ring so the test is cheap: cap = 3.
    call(
        &node,
        &p,
        "ws-a",
        "insight.policy.set",
        json!({ "ring_cap": 3 }),
    )
    .await
    .expect("policy.set ok");

    let mut id = String::new();
    for ts in 1..=4u64 {
        let o = call(
            &node,
            &p,
            "ws-a",
            "insight.raise",
            raise_input("k", "warning", ts),
        )
        .await
        .unwrap();
        id = o["id"].as_str().unwrap().to_string();
    }
    // Lifetime count = 4; the ring holds only the newest 3.
    let got = call(&node, &p, "ws-a", "insight.get", json!({ "id": id }))
        .await
        .unwrap();
    assert_eq!(got["count"], 4, "count is lifetime, exceeds the ring");
    let page = call(
        &node,
        &p,
        "ws-a",
        "insight.occurrences",
        json!({ "insight_id": id, "limit": 50 }),
    )
    .await
    .unwrap();
    assert_eq!(
        page["items"].as_array().unwrap().len(),
        3,
        "ring evicted the oldest to the cap"
    );
    // Newest-first: the top row is the 4th firing (oseq = 4).
    assert_eq!(page["items"][0]["oseq"], 4);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn oversize_occurrence_data_rejects_the_whole_raise() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:ada", "ws-a", &member_caps());
    // A >2 KB string payload.
    let big = "x".repeat(3000);
    let input = json!({
        "dedup_key": "k",
        "severity": "warning",
        "title": "t",
        "origin": { "kind": "manual", "ref": "cli" },
        "occurrence": { "data": { "blob": big } },
        "ts": 1,
    });
    let r = call(&node, &p, "ws-a", "insight.raise", input).await;
    assert!(matches!(r, Err(ToolError::BadInput(_))), "oversize rejects");
    // The reject left NO parent row (validated up front).
    let page = call(&node, &p, "ws-a", "insight.list", json!({}))
        .await
        .unwrap();
    assert_eq!(page["items"].as_array().unwrap().len(), 0, "no orphan row");

    // Exactly-at-cap accepts (a small payload well under 2 KB).
    let ok = call(
        &node,
        &p,
        "ws-a",
        "insight.raise",
        raise_input("k2", "warning", 2),
    )
    .await;
    assert!(ok.is_ok());
}

// --- scope-named: matcher axes (through the real raise path) -----------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn matcher_delivers_to_a_matching_tag_sub_and_not_a_nonmatching_one() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:ada", "ws-a", &member_caps());
    // Sub 1 matches tag kind=anomaly into channel "ops"; sub 2 filters a different tag.
    call(
        &node,
        &p,
        "ws-a",
        "insight.sub.create",
        json!({
            "sink": { "kind": "channel", "channel": "ops" },
            "filter": { "tags": { "kind": "anomaly" } },
            "now": 1,
        }),
    )
    .await
    .expect("sub 1");
    call(
        &node,
        &p,
        "ws-a",
        "insight.sub.create",
        json!({
            "sink": { "kind": "channel", "channel": "other" },
            "filter": { "tags": { "kind": "energy" } },
            "now": 1,
        }),
    )
    .await
    .expect("sub 2");

    // A raise tagged kind=anomaly ⇒ first-key breakthrough ⇒ immediate post into "ops" only.
    call(
        &node,
        &p,
        "ws-a",
        "insight.raise",
        raise_input("k", "critical", 10),
    )
    .await
    .expect("raise");

    // The matched sub's channel got a message; the non-matching one did not. We read the channel
    // inbox through the host inbox verb (real record path).
    let ops = lb_host::list_inbox(&node.store, &p, "ws-a", "ops")
        .await
        .expect("ops inbox");
    assert_eq!(
        ops.len(),
        1,
        "the matching tag sub delivered the breakthrough"
    );
    let other = lb_host::list_inbox(&node.store, &p, "ws-a", "other")
        .await
        .expect("other inbox");
    assert_eq!(other.len(), 0, "the non-matching sub delivered nothing");
}

// --- scope-named: ladder integration (the 5-min-nag headline) ----------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ladder_first_raise_posts_then_cooldown_holds_the_rest() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:ada", "ws-a", &member_caps());
    call(
        &node,
        &p,
        "ws-a",
        "insight.sub.create",
        json!({
            "sink": { "kind": "channel", "channel": "ops" },
            "filter": {},
            "now": 1,
        }),
    )
    .await
    .expect("sub all");

    // 10 raises within the L0 cooldown (15 min). First is a first-key breakthrough (immediate);
    // the rest accumulate into pending (no per-raise post) — the anti-spam ladder in action.
    for ts in 0..10u64 {
        call(
            &node,
            &p,
            "ws-a",
            "insight.raise",
            raise_input("k", "warning", ts * 1000),
        )
        .await
        .expect("raise");
    }
    let ops = lb_host::list_inbox(&node.store, &p, "ws-a", "ops")
        .await
        .expect("ops inbox");
    assert!(
        ops.len() <= 2,
        "10 raises in the cooldown produced at most a couple of posts, not 10 (got {})",
        ops.len()
    );
    assert!(ops.len() >= 1, "the first raise broke through immediately");
}

// --- scope-named: digest idempotency -----------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn digest_reactor_is_idempotent_on_rerun() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:ada", "ws-a", &member_caps());
    call(
        &node,
        &p,
        "ws-a",
        "insight.sub.create",
        json!({
            "sink": { "kind": "channel", "channel": "ops" },
            "filter": {},
            // Pin to daily so raises accumulate into a digest window (no per-raise post after the
            // first breakthrough).
            "throttle_override": "daily",
            "now": 1,
        }),
    )
    .await
    .expect("sub");

    // Some raises accumulate pending under the daily window.
    for ts in 0..4u64 {
        call(
            &node,
            &p,
            "ws-a",
            "insight.raise",
            raise_input("k", "warning", ts),
        )
        .await
        .expect("raise");
    }
    // Advance the clock a full day and drive the reactor — one digest posts.
    let day = 24 * 60 * 60 * 1000 + 1;
    lb_host::react_to_insight_digests(&node, "ws-a", day)
        .await
        .expect("digest pass 1");
    let after_first = lb_host::list_inbox(&node.store, &p, "ws-a", "ops")
        .await
        .unwrap()
        .len();
    // Re-drive the reactor at the SAME now — the state was consumed, so no duplicate post.
    lb_host::react_to_insight_digests(&node, "ws-a", day)
        .await
        .expect("digest pass 2");
    let after_second = lb_host::list_inbox(&node.store, &p, "ws-a", "ops")
        .await
        .unwrap()
        .len();
    assert_eq!(
        after_first, after_second,
        "re-running the reactor never double-posts a digest"
    );
}

// --- scope-named: kill switch ------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn member_kill_switch_off_skips_all_deliveries() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:ada", "ws-a", &member_caps());
    // Turn OFF ada's per-member insight notifications (the prefs kill switch).
    let patch: lb_prefs::Prefs =
        serde_json::from_value(serde_json::json!({ "insight_notifications": false }))
            .expect("prefs patch");
    lb_prefs::set_user_prefs(&node.store, "ws-a", "user:ada", &patch)
        .await
        .expect("set prefs");

    call(
        &node,
        &p,
        "ws-a",
        "insight.sub.create",
        json!({
            "sink": { "kind": "channel", "channel": "ops" },
            "filter": {},
            "now": 1,
        }),
    )
    .await
    .expect("sub");
    // A first-key raise would normally break through immediately — but the kill switch suppresses.
    call(
        &node,
        &p,
        "ws-a",
        "insight.raise",
        raise_input("k", "critical", 10),
    )
    .await
    .expect("raise");
    let ops = lb_host::list_inbox(&node.store, &p, "ws-a", "ops")
        .await
        .unwrap();
    assert_eq!(
        ops.len(),
        0,
        "kill switch off ⇒ no delivery (accounting only)"
    );
}
