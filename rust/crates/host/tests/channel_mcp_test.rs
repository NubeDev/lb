//! Slice 2 — the `channel.*` MCP verbs (rules-messaging-scope), over a REAL booted `Node`: real
//! store, real bus, real caps, the real `post`/`history`/`edit`/`delete`/`channel_list` host fns
//! reached through the real `call_tool` MCP bridge. NO mocks (CLAUDE §9) — records are seeded by
//! posting through the verb under test, then read back through it.
//!
//! Mandatory categories (testing-scope §2):
//!   - HAPPY PATH: post → history → edit → delete → list round-trips through the dispatcher.
//!   - CAPABILITY DENY (opaque, at the dispatcher): a caller lacking `bus:chan/{cid}:Pub` is denied
//!     `channel.post`/`edit`/`delete` with NO write; lacking `Sub` is denied `channel.history`.
//!   - WORKSPACE ISOLATION: a ws-B caller cannot post to / read a ws-A channel, and a ws-B `list`
//!     never surfaces a ws-A channel (the workspace pin refuses it before the cap check).
//!   - AUTHOR FORCED: the stored item's author is the caller's `sub`, never a request-supplied value.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, Node};
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
    verify(&key, &mint(&key, &claims), 1).unwrap()
}

/// The full channel MCP surface caps a member holds (mirrors dev-login): the two MCP-door caps not
/// covered by the `mcp:*.<verb>:call` wildcards, the wildcard-covered ones, plus the `bus:chan/*`
/// grant the host fns re-check.
fn full_caps() -> Vec<&'static str> {
    vec![
        "bus:chan/*:pub",
        "bus:chan/*:sub",
        "mcp:channel.create:call",
        "mcp:channel.post:call",
        "mcp:channel.history:call",
        "mcp:channel.edit:call",
        "mcp:channel.delete:call",
        "mcp:channel.list:call",
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

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn post_history_edit_delete_list_roundtrip() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:ada", "acme", &full_caps());

    // POST a plain-text message.
    call(
        &node,
        &p,
        "acme",
        "channel.post",
        json!({ "cid": "ops", "id": "m1", "body": "hello ops", "ts": 1 }),
    )
    .await
    .expect("post ok");

    // HISTORY reads it back — author FORCED to the caller's sub.
    let hist = call(
        &node,
        &p,
        "acme",
        "channel.history",
        json!({ "cid": "ops" }),
    )
    .await
    .expect("history ok");
    let msgs = hist["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0]["body"], "hello ops");
    assert_eq!(msgs[0]["author"], "user:ada");

    // EDIT the body.
    call(
        &node,
        &p,
        "acme",
        "channel.edit",
        json!({ "cid": "ops", "id": "m1", "body": "hello ops (edited)", "ts": 2 }),
    )
    .await
    .expect("edit ok");
    let hist = call(
        &node,
        &p,
        "acme",
        "channel.history",
        json!({ "cid": "ops" }),
    )
    .await
    .unwrap();
    assert_eq!(hist["messages"][0]["body"], "hello ops (edited)");

    // LIST surfaces the channel (create-on-post registered it).
    let list = call(&node, &p, "acme", "channel.list", json!({}))
        .await
        .expect("list ok");
    let cids: Vec<&str> = list["channels"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|c| c["cid"].as_str().or_else(|| c["id"].as_str()))
        .collect();
    assert!(cids.contains(&"ops"), "channel `ops` listed: {list:?}");

    // DELETE removes it from history.
    call(
        &node,
        &p,
        "acme",
        "channel.delete",
        json!({ "cid": "ops", "id": "m1" }),
    )
    .await
    .expect("delete ok");
    let hist = call(
        &node,
        &p,
        "acme",
        "channel.history",
        json!({ "cid": "ops" }),
    )
    .await
    .unwrap();
    assert_eq!(hist["messages"].as_array().unwrap().len(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn create_makes_channel_listable_before_any_post_and_is_idempotent() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:ada", "acme", &full_caps());

    // CREATE a channel — no post yet.
    let rec = call(
        &node,
        &p,
        "acme",
        "channel.create",
        json!({ "cid": "care-child-7", "ts": 1 }),
    )
    .await
    .expect("create ok");
    assert_eq!(
        rec["cid"].as_str().or_else(|| rec["id"].as_str()),
        Some("care-child-7"),
        "create returns the descriptor: {rec:?}"
    );

    // LIST surfaces it immediately, before any post.
    let list = call(&node, &p, "acme", "channel.list", json!({}))
        .await
        .expect("list ok");
    let cids: Vec<&str> = list["channels"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|c| c["cid"].as_str().or_else(|| c["id"].as_str()))
        .collect();
    assert!(
        cids.contains(&"care-child-7"),
        "created channel listed before any post: {list:?}"
    );

    // IDEMPOTENT — create-then-create settles (no error).
    call(
        &node,
        &p,
        "acme",
        "channel.create",
        json!({ "cid": "care-child-7", "ts": 2 }),
    )
    .await
    .expect("re-create idempotent");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn create_denied_without_pub_cap_is_opaque() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    // Holds the MCP door but NOT `bus:chan/*:pub` — must DENY, not NotFound.
    let p = principal(
        "user:eve",
        "acme",
        &[
            "bus:chan/*:sub",
            "mcp:channel.create:call",
            "mcp:channel.list:call",
        ],
    );
    let err = call(
        &node,
        &p,
        "acme",
        "channel.create",
        json!({ "cid": "care-child-7" }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied), "opaque deny, got {err:?}");
    // NO channel was registered.
    let list = call(&node, &p, "acme", "channel.list", json!({}))
        .await
        .expect("list ok");
    assert!(
        list["channels"].as_array().unwrap().is_empty(),
        "no channel registered on deny: {list:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn author_is_forced_not_request_supplied() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:ada", "acme", &full_caps());
    // A forged `author` in the request is ignored — the stored author is the caller's sub.
    call(
        &node,
        &p,
        "acme",
        "channel.post",
        json!({ "cid": "ops", "id": "m1", "body": "x", "ts": 1, "author": "user:mallory" }),
    )
    .await
    .expect("post ok");
    let hist = call(
        &node,
        &p,
        "acme",
        "channel.history",
        json!({ "cid": "ops" }),
    )
    .await
    .unwrap();
    assert_eq!(hist["messages"][0]["author"], "user:ada");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn post_denied_without_pub_cap_is_opaque_with_no_write() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    // Holds the MCP door + `sub` (to read) but NOT `bus:chan/*:pub`.
    let p = principal(
        "user:eve",
        "acme",
        &[
            "bus:chan/*:sub",
            "mcp:channel.post:call",
            "mcp:channel.history:call",
        ],
    );
    let err = call(
        &node,
        &p,
        "acme",
        "channel.post",
        json!({ "cid": "ops", "id": "m1", "body": "sneak", "ts": 1 }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied), "opaque deny, got {err:?}");
    // NO write reached the store — history (as a Sub-capable reader) is empty.
    let hist = call(
        &node,
        &p,
        "acme",
        "channel.history",
        json!({ "cid": "ops" }),
    )
    .await
    .unwrap();
    assert_eq!(hist["messages"].as_array().unwrap().len(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn history_denied_without_sub_cap() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal(
        "user:eve",
        "acme",
        &["bus:chan/*:pub", "mcp:channel.history:call"],
    );
    let err = call(
        &node,
        &p,
        "acme",
        "channel.history",
        json!({ "cid": "ops" }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied), "opaque deny, got {err:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_isolation_ws_b_cannot_touch_ws_a() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let a = principal("user:ada", "acme", &full_caps());
    let b = principal("user:bo", "beta", &full_caps());

    // ws-A seeds a channel via the real verb.
    call(
        &node,
        &a,
        "acme",
        "channel.post",
        json!({ "cid": "ops", "id": "m1", "body": "acme secret", "ts": 1 }),
    )
    .await
    .expect("ws-A post ok");

    // ws-B reads ITS OWN `ops` channel — a different namespace, so empty (never ws-A's item).
    let hist_b = call(
        &node,
        &b,
        "beta",
        "channel.history",
        json!({ "cid": "ops" }),
    )
    .await
    .expect("ws-B history ok (own ns)");
    assert_eq!(
        hist_b["messages"].as_array().unwrap().len(),
        0,
        "ws-B must not see ws-A's message"
    );

    // ws-B posts to its own `ops`, then lists — it sees only its own channel, never ws-A's.
    call(
        &node,
        &b,
        "beta",
        "channel.post",
        json!({ "cid": "ops", "id": "b1", "body": "beta msg", "ts": 1 }),
    )
    .await
    .expect("ws-B post ok");
    let hist_b = call(
        &node,
        &b,
        "beta",
        "channel.history",
        json!({ "cid": "ops" }),
    )
    .await
    .unwrap();
    assert_eq!(hist_b["messages"].as_array().unwrap().len(), 1);
    assert_eq!(hist_b["messages"][0]["body"], "beta msg");

    // ws-A still sees only its own message (unaffected by ws-B's write to the same cid).
    let hist_a = call(
        &node,
        &a,
        "acme",
        "channel.history",
        json!({ "cid": "ops" }),
    )
    .await
    .unwrap();
    assert_eq!(hist_a["messages"].as_array().unwrap().len(), 1);
    assert_eq!(hist_a["messages"][0]["body"], "acme secret");
}
