//! `reminder.fire` — the gated, idempotent **run-now** verb through the REAL MCP bridge
//! (`lb_host::call_tool`), plus the catalog integration for the `reminder.create` / `reminder.list`
//! palette commands. All real: an embedded `Node` (real SurrealDB `mem://` + Zenoh), real caps, the
//! real shipped fire path — no mocks (testing §0).
//!
//! Coverage (mirrors `reminders_mcp_test.rs` / `agent_runtimes_test.rs`):
//!   - a real firing writes the action's side effect (a real `lb_inbox` item);
//!   - CAPABILITY-DENY (§2.1): no `mcp:reminder.fire:call` → opaque `ToolError::Denied`;
//!   - WORKSPACE-ISOLATION (§2.2): a ws-B caller can't fire a ws-A reminder even with the leaked id;
//!   - IDEMPOTENCY: two run-now clicks in the same logical `now` → exactly ONE action effect;
//!   - CATALOG: `reminder.create` / `reminder.list` appear only WITH their `mcp:reminder.<verb>:call`.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, tools_catalog, Node};
use lb_mcp::ToolError;
use serde_json::{json, Value};
use std::sync::Arc;

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

fn channel_post_action(channel: &str, body: &str) -> Value {
    json!({ "kind": "channel-post", "channel": channel, "body": body })
}

/// Grant `cap` to `user` in `ws` directly in the durable grant store — this is how the fire-time
/// re-resolve sees the stored principal's CURRENT caps (the action's own gate).
async fn grant(store: &lb_store::Store, ws: &str, user: &str, cap: &str) {
    lb_authz::grant_assign(store, ws, &lb_authz::Subject::User(user.to_string()), cap)
        .await
        .unwrap();
}

/// Create a channel-post reminder `id` in `ws` as `p` (schedule irrelevant to run-now).
async fn create_reminder(node: &Arc<Node>, p: &Principal, ws: &str, id: &str, channel: &str) {
    call_tool(
        node,
        p,
        ws,
        "reminder.create",
        &json!({
            "id": id, "schedule": "0 8 * * 1",
            "action": channel_post_action(channel, "run now body"), "ts": 1
        })
        .to_string(),
    )
    .await
    .expect("create with the grant");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn fires_a_reminder_now_with_the_cap() {
    // A real run-now firing: the action's own cap (`bus:chan/team:pub`) is granted to the stored
    // principal, so the fire-time re-check passes and a real `lb_inbox` item lands in the channel.
    let ws = "fire-now-ok";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(
        "user:ada",
        ws,
        &[
            "mcp:reminder.create:call",
            "mcp:reminder.get:call",
            "mcp:reminder.fire:call",
        ],
    );
    grant(&node.store, ws, "user:ada", "bus:chan/team:pub").await;

    create_reminder(&node, &p, ws, "standup", "team").await;

    let out = call_tool(
        &node,
        &p,
        ws,
        "reminder.fire",
        &json!({ "id": "standup", "ts": 5000 }).to_string(),
    )
    .await
    .expect("fire with the grant");
    let r: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(r["fired"], true);
    assert_eq!(r["scheduled_ts"], 5000);

    // The real side effect: a durable inbox item in the channel (the shipped fire path).
    let items = lb_inbox::list(&node.store, ws, "team").await.unwrap();
    assert_eq!(items.len(), 1, "the firing wrote a real inbox item");
    assert_eq!(items[0].body, "run now body");
    assert_eq!(items[0].author, "user:ada");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn denies_fire_without_cap() {
    // CAPABILITY-DENY (mandatory): a caller who can create/get but NOT fire is refused opaquely at the
    // MCP gate, before any dispatch.
    let ws = "fire-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(
        "user:ada",
        ws,
        &["mcp:reminder.create:call", "mcp:reminder.get:call"],
    );
    grant(&node.store, ws, "user:ada", "bus:chan/team:pub").await;
    create_reminder(&node, &p, ws, "standup", "team").await;

    let err = call_tool(
        &node,
        &p,
        ws,
        "reminder.fire",
        &json!({ "id": "standup", "ts": 5000 }).to_string(),
    )
    .await
    .expect_err("fire denied without the cap");
    assert!(
        matches!(err, ToolError::Denied),
        "opaque denial, got {err:?}"
    );

    // No effect produced (the gate ran before dispatch).
    assert!(lb_inbox::list(&node.store, ws, "team")
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn fire_is_workspace_isolated() {
    // WORKSPACE-ISOLATION (mandatory): ws-A creates "x"; a ws-B principal (with the fire cap IN ws-B)
    // firing `{id:"x"}` cannot reach ws-A's reminder — the store namespace is the token's ws, so the
    // id reads as absent → NotFound, no effect. Even a leaked id can't cross the wall.
    let node = Arc::new(Node::boot().await.unwrap());
    let a = principal(
        "user:ada",
        "fire-iso-a",
        &["mcp:reminder.create:call", "mcp:reminder.get:call"],
    );
    grant(&node.store, "fire-iso-a", "user:ada", "bus:chan/team:pub").await;
    create_reminder(&node, &a, "fire-iso-a", "x", "team").await;

    let b = principal(
        "user:bob",
        "fire-iso-b",
        &["mcp:reminder.fire:call", "mcp:reminder.get:call"],
    );
    let err = call_tool(
        &node,
        &b,
        "fire-iso-b",
        "reminder.fire",
        &json!({ "id": "x", "ts": 5000 }).to_string(),
    )
    .await
    .expect_err("ws-B cannot fire ws-A's reminder");
    assert!(
        matches!(err, ToolError::NotFound),
        "leaked id reads as absent in ws-B, got {err:?}"
    );

    // ws-A's channel got NO item (ws-B never fired it).
    assert!(lb_inbox::list(&node.store, "fire-iso-a", "team")
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn double_fire_same_instant_is_idempotent() {
    // IDEMPOTENCY (mandatory): two run-now calls at the SAME logical `now` → exactly ONE action effect.
    // The deterministic job id for (reminder, now) dedupes the second click (fired:false).
    let ws = "fire-idem";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(
        "user:ada",
        ws,
        &[
            "mcp:reminder.create:call",
            "mcp:reminder.get:call",
            "mcp:reminder.fire:call",
        ],
    );
    grant(&node.store, ws, "user:ada", "bus:chan/team:pub").await;
    create_reminder(&node, &p, ws, "standup", "team").await;

    let fire = |now: u64| {
        let node = node.clone();
        let p = p.clone();
        async move {
            call_tool(
                &node,
                &p,
                ws,
                "reminder.fire",
                &json!({ "id": "standup", "ts": now }).to_string(),
            )
            .await
            .unwrap()
        }
    };

    let first: Value = serde_json::from_str(&fire(7000).await).unwrap();
    assert_eq!(first["fired"], true, "first click fires");

    let second: Value = serde_json::from_str(&fire(7000).await).unwrap();
    assert_eq!(
        second["fired"], false,
        "second click at the same instant is a no-op"
    );

    // Exactly ONE inbox item, ever (one action effect for the instant).
    let items = lb_inbox::list(&node.store, ws, "team").await.unwrap();
    assert_eq!(items.len(), 1, "double-fire → one action effect");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn create_accepts_the_flat_descriptor_form_no_nested_action_no_id() {
    // BACKEND-DRIVEN: the generic palette posts the descriptor's FLAT form straight from the form —
    // `schedule` + `action_kind:"channel-post"` + `channel` + `body`, with NO nested `action` and NO
    // `id` (the UI derives neither). The host assembles the `Action`, supplies `now`, and derives a
    // stable id server-side. We assert the real stored reminder via `reminder.get`.
    let ws = "create-flat";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(
        "user:ada",
        ws,
        &[
            "mcp:reminder.create:call",
            "mcp:reminder.list:call",
            "mcp:reminder.get:call",
        ],
    );

    let out = call_tool(
        &node,
        &p,
        ws,
        "reminder.create",
        &json!({
            "schedule": "0 8 * * 1",
            "action_kind": "channel-post",
            "channel": "team",
            "body": "flat-form body"
        })
        .to_string(),
    )
    .await
    .expect("flat-form create with the grant");
    let created: Value = serde_json::from_str(&out).unwrap();
    // The host derived a stable id (no uuid) and the right channel-post action.
    let id = created["id"].as_str().expect("derived id");
    assert!(
        id.starts_with("reminder-post-team-"),
        "stable derived id: {id}"
    );
    assert_eq!(created["action"]["kind"], "channel-post");
    assert_eq!(created["action"]["channel"], "team");
    assert_eq!(created["action"]["body"], "flat-form body");

    // The reminder is really persisted (read it back through the get verb).
    let got = call_tool(
        &node,
        &p,
        ws,
        "reminder.get",
        &json!({ "id": id }).to_string(),
    )
    .await
    .expect("get the just-created reminder");
    let got: Value = serde_json::from_str(&got).unwrap();
    assert_eq!(got["reminder"]["action"]["kind"], "channel-post");
    assert_eq!(got["reminder"]["schedule"], "0 8 * * 1");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn create_still_accepts_the_nested_action_form() {
    // BACKWARD COMPAT: the nested `action:{kind,…}` wire form (the reminder engine + existing callers)
    // still creates a real reminder — the flat-form support is additive, not a replacement.
    let ws = "create-nested";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(
        "user:ada",
        ws,
        &["mcp:reminder.create:call", "mcp:reminder.get:call"],
    );
    call_tool(
        &node,
        &p,
        ws,
        "reminder.create",
        &json!({
            "id": "nested-1", "schedule": "0 8 * * 1",
            "action": channel_post_action("team", "nested body"), "ts": 1
        })
        .to_string(),
    )
    .await
    .expect("nested-form create");
    let got = call_tool(
        &node,
        &p,
        ws,
        "reminder.get",
        &json!({ "id": "nested-1" }).to_string(),
    )
    .await
    .expect("get nested");
    let got: Value = serde_json::from_str(&got).unwrap();
    assert_eq!(got["reminder"]["action"]["kind"], "channel-post");
    assert_eq!(got["reminder"]["action"]["body"], "nested body");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn write_verbs_default_ts_when_absent() {
    // REGRESSION (debugging/reminders/reminder-write-verbs-require-ts.md): the 100%-backend-driven row
    // controls send a `ts`-FREE `argsTemplate` (the frontend is generic; the gateway forwards args
    // verbatim). `reminder.update/delete/fire` USED to hard-require `ts` (`u64_arg`) — even though
    // `create` already defaulted it — so every row control was rejected `missing u64 arg: ts` before any
    // effect. This drives each write verb the EXACT way a control does (no `ts`) and asserts the real
    // effect. Fails before the fix; passes after.
    let ws = "write-no-ts";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(
        "user:ada",
        ws,
        &[
            "mcp:reminder.create:call",
            "mcp:reminder.get:call",
            "mcp:reminder.update:call",
            "mcp:reminder.fire:call",
            "mcp:reminder.delete:call",
        ],
    );
    grant(&node.store, ws, "user:ada", "bus:chan/team:pub").await;
    create_reminder(&node, &p, ws, "standup", "team").await;

    // update WITHOUT ts (the pause switch's shape) → succeeds and flips `enabled`.
    let out = call_tool(
        &node,
        &p,
        ws,
        "reminder.update",
        &json!({ "id": "standup", "enabled": false }).to_string(),
    )
    .await
    .expect("update without ts defaults to the host clock");
    let updated: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(updated["enabled"], false, "the pause toggle took effect");

    // fire WITHOUT ts (the run-now button's shape) → succeeds and produces the one action effect.
    let out = call_tool(
        &node,
        &p,
        ws,
        "reminder.fire",
        &json!({ "id": "standup" }).to_string(),
    )
    .await
    .expect("fire without ts defaults to the host clock");
    let fired: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(fired["fired"], true, "run-now fired");
    assert_eq!(
        lb_inbox::list(&node.store, ws, "team").await.unwrap().len(),
        1,
        "run-now produced exactly one action effect"
    );

    // delete WITHOUT ts (the delete button's shape) → succeeds and tombstones (get returns none).
    call_tool(
        &node,
        &p,
        ws,
        "reminder.delete",
        &json!({ "id": "standup" }).to_string(),
    )
    .await
    .expect("delete without ts defaults to the host clock");
    let got = call_tool(
        &node,
        &p,
        ws,
        "reminder.get",
        &json!({ "id": "standup" }).to_string(),
    )
    .await
    .expect("get after delete");
    let got: Value = serde_json::from_str(&got).unwrap();
    assert!(
        got["reminder"].is_null(),
        "the delete tombstoned the reminder"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn catalog_shows_reminder_create_and_list_only_with_their_cap() {
    // CATALOG INTEGRATION: each reminder command appears only for a caller holding its own
    // `mcp:reminder.<verb>:call` (the descriptor name IS the gate) — absent otherwise, no existence leak.
    const CATALOG: &str = "mcp:tools.catalog:call";
    let node = Arc::new(Node::boot().await.unwrap());
    let ws = "fire-catalog";

    // WITH create + list + fire → all three commands present, and create carries the cron widget hint.
    let member = principal(
        "user:ada",
        ws,
        &[
            CATALOG,
            "mcp:reminder.create:call",
            "mcp:reminder.list:call",
            "mcp:reminder.fire:call",
        ],
    );
    let cat = tools_catalog(&node, &member, ws).await.expect("catalog");
    let create = cat
        .tools
        .iter()
        .find(|t| t.name == "reminder.create")
        .expect("a member with mcp:reminder.create:call sees the create command");
    let schema = create.input_schema.as_ref().expect("create has a schema");
    assert_eq!(schema["properties"]["schedule"]["x-lb"]["widget"], "cron");
    // `reminder.list` carries its response render (the OUTPUT contract the palette posts verbatim).
    let list = cat
        .tools
        .iter()
        .find(|t| t.name == "reminder.list")
        .expect("list command present");
    let render = list.result.as_ref().expect("list declares a render");
    assert_eq!(render["view"], "table");
    assert_eq!(render["source"]["tool"], "reminder.list");
    assert!(cat.tools.iter().any(|t| t.name == "reminder.fire"));

    // WITHOUT any reminder cap → none of the three commands appear (absent, not greyed).
    let denied = principal("user:eve", ws, &[CATALOG]);
    let cat2 = tools_catalog(&node, &denied, ws).await.expect("catalog");
    assert!(
        !cat2.tools.iter().any(|t| t.name == "reminder.create"),
        "no create cap → no create command"
    );
    assert!(
        !cat2.tools.iter().any(|t| t.name == "reminder.list"),
        "no list cap → no list command"
    );
    assert!(
        !cat2.tools.iter().any(|t| t.name == "reminder.fire"),
        "no fire cap → no fire command"
    );
}
