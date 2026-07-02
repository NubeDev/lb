//! The reminders CRUD surface through the REAL MCP bridge (`lb_host::call_tool`) — the same entry
//! the gateway's `POST /mcp/call` forwards. Proves every verb the scope named (create / update /
//! delete / get / list) round-trips end to end, each is denied opaquely without its grant, and the
//! workspace wall holds at list/get (a ws-B caller never sees ws-A's reminders).

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, Node};
use lb_mcp::ToolError;
use lb_reminders::Action;
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

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn create_get_list_update_delete_round_trip() {
    let ws = "rem-crud";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(
        "user:ada",
        ws,
        &[
            "mcp:reminder.create:call",
            "mcp:reminder.get:call",
            "mcp:reminder.list:call",
            "mcp:reminder.update:call",
            "mcp:reminder.delete:call",
        ],
    );

    // create (max_runs=3, channel-post) — nextAttemptTs is the next future slot strictly after now.
    let now: u64 = 1_704_067_200; // Mon 2024-01-01 00:00 UTC
    let out = call_tool(
        &node,
        &p,
        ws,
        "reminder.create",
        &json!({
            "id": "standup",
            "schedule": "0 8 * * 1",
            "max_runs": 3,
            "action": channel_post_action("team", "standup time"),
            "ts": now,
        })
        .to_string(),
    )
    .await
    .expect("create with the grant");
    let r: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(r["id"], "standup");
    assert_eq!(r["schedule"], "0 8 * * 1");
    assert_eq!(r["maxRuns"], 3);
    assert_eq!(r["runs"], 0);
    assert!(r["enabled"].as_bool().unwrap());
    assert_eq!(r["status"], "active");
    assert_eq!(r["nextAttemptTs"], 1_704_096_000); // Mon 08:00 same day

    // get reads it back.
    let out = call_tool(
        &node,
        &p,
        ws,
        "reminder.get",
        &json!({ "id": "standup" }).to_string(),
    )
    .await
    .unwrap();
    let g: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(g["reminder"]["id"], "standup");

    // list returns it (one row).
    let out = call_tool(&node, &p, ws, "reminder.list", "{}")
        .await
        .unwrap();
    let l: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(l["reminders"].as_array().unwrap().len(), 1);

    // update: pause (enabled=false) + reschedule to Sunday.
    let out = call_tool(
        &node,
        &p,
        ws,
        "reminder.update",
        &json!({ "id": "standup", "enabled": false, "schedule": "0 8 * * 0", "ts": now })
            .to_string(),
    )
    .await
    .unwrap();
    let u: Value = serde_json::from_str(&out).unwrap();
    assert!(!u["enabled"].as_bool().unwrap());
    assert_eq!(u["schedule"], "0 8 * * 0");

    // delete tombstones it — list is now empty, get returns null.
    call_tool(
        &node,
        &p,
        ws,
        "reminder.delete",
        &json!({ "id": "standup", "ts": now }).to_string(),
    )
    .await
    .unwrap();
    let out = call_tool(&node, &p, ws, "reminder.list", "{}")
        .await
        .unwrap();
    let l: Value = serde_json::from_str(&out).unwrap();
    assert!(
        l["reminders"].as_array().unwrap().is_empty(),
        "deleted → not listed"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn each_verb_is_denied_without_its_grant() {
    // MANDATORY capability-deny, per verb: a caller with NO reminder grants is refused opaquely at
    // the MCP gate, before any store access (no record created). One deny-test per verb.
    let ws = "rem-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    let nobody = principal("user:eve", ws, &[]);

    for (tool, input) in [
        (
            "reminder.create",
            json!({
                "id": "x", "schedule": "0 8 * * 1",
                "action": channel_post_action("c", "b"), "ts": 1
            }),
        ),
        ("reminder.update", json!({ "id": "x", "ts": 1 })),
        ("reminder.delete", json!({ "id": "x", "ts": 1 })),
        ("reminder.get", json!({ "id": "x" })),
        ("reminder.list", json!({})),
    ] {
        let err = call_tool(&node, &nobody, ws, tool, &input.to_string())
            .await
            .expect_err(&format!("{tool} denied without grant"));
        assert!(
            matches!(err, ToolError::Denied),
            "{tool}: opaque denial, got {err:?}"
        );
    }

    // No reminder was created by the denied create.
    let listing = lb_reminders::list(&node.store, ws).await.unwrap();
    assert!(listing.is_empty(), "no record survived a denied create");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_isolation_list_and_get_never_cross_the_wall() {
    // MANDATORY workspace-isolation at the MCP surface: ws-A creates a reminder; ws-B's list/get
    // never see it (the store namespace is selected from the caller's ws).
    let node = Arc::new(Node::boot().await.unwrap());
    let a = principal(
        "user:ada",
        "rem-iso-a",
        &[
            "mcp:reminder.create:call",
            "mcp:reminder.list:call",
            "mcp:reminder.get:call",
        ],
    );
    let b = principal(
        "user:bob",
        "rem-iso-b",
        &["mcp:reminder.list:call", "mcp:reminder.get:call"],
    );

    call_tool(
        &node,
        &a,
        "rem-iso-a",
        "reminder.create",
        &json!({
            "id": "secret", "schedule": "0 8 * * 1",
            "action": channel_post_action("c", "b"), "ts": 1
        })
        .to_string(),
    )
    .await
    .unwrap();

    // ws-B lists its own (empty) — never ws-A's reminder.
    let out = call_tool(&node, &b, "rem-iso-b", "reminder.list", "{}")
        .await
        .unwrap();
    let l: Value = serde_json::from_str(&out).unwrap();
    assert!(
        l["reminders"].as_array().unwrap().is_empty(),
        "ISO LEAK: ws-B saw ws-A's reminder"
    );

    // ws-B get for the same id returns null (different namespace).
    let out = call_tool(
        &node,
        &b,
        "rem-iso-b",
        "reminder.get",
        &json!({ "id": "secret" }).to_string(),
    )
    .await
    .unwrap();
    let g: Value = serde_json::from_str(&out).unwrap();
    assert!(
        g["reminder"].is_null(),
        "ISO LEAK: ws-B read ws-A's reminder"
    );

    // ws-A sees its own.
    let out = call_tool(&node, &a, "rem-iso-a", "reminder.list", "{}")
        .await
        .unwrap();
    let l: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(l["reminders"].as_array().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn list_status_and_limit_filter_over_the_ws_read() {
    // The D3 `list` filter grammar (shared minimal core) at the MCP surface: `status` selects on the
    // `enabled` flag, `limit` truncates the sorted head. Both are applied host-side over the ws read.
    let ws = "rem-filter";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(
        "user:ada",
        ws,
        &[
            "mcp:reminder.create:call",
            "mcp:reminder.list:call",
            "mcp:reminder.update:call",
        ],
    );
    let now: u64 = 1;

    // Two reminders; pause the second (enabled=false).
    for id in ["a", "b"] {
        call_tool(
            &node,
            &p,
            ws,
            "reminder.create",
            &json!({
                "id": id, "schedule": "0 8 * * 1",
                "action": channel_post_action("c", "b"), "ts": now,
            })
            .to_string(),
        )
        .await
        .unwrap();
    }
    call_tool(
        &node,
        &p,
        ws,
        "reminder.update",
        &json!({ "id": "b", "enabled": false, "ts": now }).to_string(),
    )
    .await
    .unwrap();

    // status=enabled → only `a`.
    let out = call_tool(
        &node,
        &p,
        ws,
        "reminder.list",
        &json!({ "status": "enabled" }).to_string(),
    )
    .await
    .unwrap();
    let l: Value = serde_json::from_str(&out).unwrap();
    let ids: Vec<&str> = l["reminders"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["a"], "status=enabled selects the enabled one");

    // status=disabled → only `b`.
    let out = call_tool(
        &node,
        &p,
        ws,
        "reminder.list",
        &json!({ "status": "disabled" }).to_string(),
    )
    .await
    .unwrap();
    let l: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(l["reminders"].as_array().unwrap().len(), 1);
    assert_eq!(l["reminders"][0]["id"], "b");

    // limit=1 → one row from the full set.
    let out = call_tool(
        &node,
        &p,
        ws,
        "reminder.list",
        &json!({ "limit": 1 }).to_string(),
    )
    .await
    .unwrap();
    let l: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        l["reminders"].as_array().unwrap().len(),
        1,
        "limit truncates"
    );

    // A garbage status is author feedback (BadInput), not an opaque deny.
    let err = call_tool(
        &node,
        &p,
        ws,
        "reminder.list",
        &json!({ "status": "paused" }).to_string(),
    )
    .await
    .expect_err("bad status filter rejected");
    assert!(matches!(err, ToolError::BadInput(_)), "got {err:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn bad_cron_at_create_is_bad_input_not_denied() {
    // A malformed schedule is author feedback (BadInput), not an opaque denial — the caller IS
    // authorized; the input is just wrong.
    let ws = "rem-badcron";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:ada", ws, &["mcp:reminder.create:call"]);
    let err = call_tool(
        &node,
        &p,
        ws,
        "reminder.create",
        &json!({
            "id": "x", "schedule": "not a cron",
            "action": channel_post_action("c", "b"), "ts": 1
        })
        .to_string(),
    )
    .await
    .expect_err("bad cron rejected");
    assert!(matches!(err, ToolError::BadInput(_)), "got {err:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn unknown_verb_is_opaque_denied_at_the_gate() {
    // An unknown `reminder.<verb>` is gated by `mcp:reminder.<verb>:call` FIRST — a caller without
    // that (non-existent) grant gets an opaque `Denied`, indistinguishable from a missing tool. The
    // MCP contract never reveals verb existence to an unauthorized caller; the bridge's `NotFound`
    // arm is defense-in-depth, only reachable by a caller holding a cap for a verb that doesn't exist.
    let ws = "rem-404";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:ada", ws, &["mcp:reminder.create:call"]);
    let err = call_tool(&node, &p, ws, "reminder.poke", "{}")
        .await
        .expect_err("unknown verb refused");
    assert!(
        matches!(err, ToolError::Denied),
        "opaque denial, got {err:?}"
    );
    let _ = Action::ChannelPost {
        channel: "".into(),
        body: "".into(),
    }; // exercise tag enum build
}
