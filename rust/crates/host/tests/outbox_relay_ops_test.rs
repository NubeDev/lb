//! The sidecar-drivable relay verbs (`outbox.due` / `outbox.mark_delivered` / `outbox.mark_failed`),
//! driven through `lb_host::call_tool` — the SAME bridge entry a native sidecar's callback reaches (a
//! `POST /mcp/call` → `call_tool`). No mocks (CLAUDE rule 9): a real `Node` + embedded store + the real
//! outbox verbs. These prove the relay surface a native driver (e.g. the ROS sidecar) uses to deliver
//! its own must-deliver effects out-of-process:
//!   - **cap deny:** a caller without `mcp:outbox.due:call` (etc.) is refused (opaque) — a normal
//!     enqueue/status grantee cannot drive delivery.
//!   - **workspace isolation:** a ws-B relay's `due` sees NONE of ws-A's effects (the hard wall §7).
//!   - **target filter:** a `ros` relay's `due` excludes a `github`-targeted effect.
//!   - **delivery lifecycle:** enqueue → due → mark_delivered → no longer due (never double-sent).
//!   - **retry + backoff:** mark_failed → not due until backoff elapses → due again (never lost).

use std::sync::Arc;

use lb_auth::Principal;
use lb_auth::{mint, verify, Claims, Role, SigningKey};
use lb_host::{call_tool, Node};
use serde_json::{json, Value};

const ENQUEUE: &str = "mcp:outbox.enqueue:call";
const DUE: &str = "mcp:outbox.due:call";
const MARK_OK: &str = "mcp:outbox.mark_delivered:call";
const MARK_FAIL: &str = "mcp:outbox.mark_failed:call";

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "ext:ros".into(),
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
    args: Value,
) -> Result<Value, String> {
    let out = call_tool(node, p, ws, tool, &args.to_string())
        .await
        .map_err(|e| e.to_string())?;
    Ok(serde_json::from_str(&out).unwrap_or(Value::String(out)))
}

/// Enqueue an effect as a fully-granted relay principal (a helper the lifecycle tests reuse).
async fn enqueue(node: &Arc<Node>, p: &Principal, ws: &str, id: &str, target: &str, ts: u64) {
    call(
        node,
        p,
        ws,
        "outbox.enqueue",
        json!({ "id": id, "target": target, "action": "point.write", "payload": "{}", "ts": ts }),
    )
    .await
    .expect("enqueue ok");
}

fn due_ids(v: &Value) -> Vec<String> {
    v["effects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn due_is_denied_without_the_cap() {
    let ws = "relay-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    // A caller that can enqueue but NOT drive the relay.
    let staged = principal(ws, &[ENQUEUE]);
    enqueue(&node, &staged, ws, "e1", "ros", 1).await;

    let no_due = principal(ws, &[ENQUEUE]); // lacks mcp:outbox.due:call
    let err = call(&node, &no_due, ws, "outbox.due", json!({ "now": 100 })).await;
    assert!(err.is_err(), "due without the cap is denied");

    let no_mark = principal(ws, &[ENQUEUE]);
    assert!(
        call(
            &node,
            &no_mark,
            ws,
            "outbox.mark_delivered",
            json!({ "id": "e1" })
        )
        .await
        .is_err(),
        "mark_delivered without the cap is denied"
    );
    assert!(
        call(
            &node,
            &no_mark,
            ws,
            "outbox.mark_failed",
            json!({ "id": "e1", "now": 1 })
        )
        .await
        .is_err(),
        "mark_failed without the cap is denied"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn due_is_workspace_isolated() {
    let node = Arc::new(Node::boot().await.unwrap());
    let caps = [ENQUEUE, DUE, MARK_OK, MARK_FAIL];
    let a = principal("ws-a", &caps);
    enqueue(&node, &a, "ws-a", "a-effect", "ros", 1).await;

    // ws-B, full relay grant, sees NONE of ws-A's effects.
    let b = principal("ws-b", &caps);
    let due_b = call(&node, &b, "ws-b", "outbox.due", json!({ "now": 100 }))
        .await
        .expect("B due ok");
    assert!(
        due_ids(&due_b).is_empty(),
        "ws-B relay must not see ws-A's effects: {due_b}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn due_filters_by_target() {
    let ws = "relay-filter";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(ws, &[ENQUEUE, DUE]);
    enqueue(&node, &p, ws, "ros-1", "ros", 1).await;
    enqueue(&node, &p, ws, "gh-1", "github", 1).await;

    // No filter → both effects are due.
    let all = call(&node, &p, ws, "outbox.due", json!({ "now": 100 }))
        .await
        .unwrap();
    assert_eq!(due_ids(&all).len(), 2, "both effects due unfiltered");

    // Target filter → only the ros effect (a ros relay never sees the github one).
    let ros = call(
        &node,
        &p,
        ws,
        "outbox.due",
        json!({ "target": "ros", "now": 100 }),
    )
    .await
    .unwrap();
    assert_eq!(due_ids(&ros), vec!["ros-1"], "target filter narrows to ros");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn lifecycle_enqueue_due_deliver_not_due() {
    let ws = "relay-life";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(ws, &[ENQUEUE, DUE, MARK_OK]);
    enqueue(&node, &p, ws, "e1", "ros", 1).await;

    let due1 = call(&node, &p, ws, "outbox.due", json!({ "now": 100 }))
        .await
        .unwrap();
    assert_eq!(due_ids(&due1), vec!["e1"], "fresh effect is due");

    // Deliver it — now terminal, no longer due (never double-sent).
    call(
        &node,
        &p,
        ws,
        "outbox.mark_delivered",
        json!({ "id": "e1" }),
    )
    .await
    .expect("mark_delivered ok");
    let due2 = call(&node, &p, ws, "outbox.due", json!({ "now": 200 }))
        .await
        .unwrap();
    assert!(
        due_ids(&due2).is_empty(),
        "delivered effect is not due again: {due2}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn retry_backoff_then_due_again() {
    let ws = "relay-retry";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(ws, &[ENQUEUE, DUE, MARK_FAIL]);
    enqueue(&node, &p, ws, "e1", "ros", 1).await;

    // First attempt fails at now=10 → backoff pushes next_attempt_ts out; status stays schedulable.
    let failed = call(
        &node,
        &p,
        ws,
        "outbox.mark_failed",
        json!({ "id": "e1", "now": 10 }),
    )
    .await
    .expect("mark_failed ok");
    assert_eq!(
        failed["status"], "failed",
        "one failure is not yet dead-lettered"
    );

    // Immediately after (now=10) it is NOT due — waiting out its backoff (owed, not due).
    let soon = call(&node, &p, ws, "outbox.due", json!({ "now": 10 }))
        .await
        .unwrap();
    assert!(
        due_ids(&soon).is_empty(),
        "failed effect waits out backoff: {soon}"
    );

    // Far in the future, the backoff has elapsed → due again (at-least-once retry holds).
    let later = call(&node, &p, ws, "outbox.due", json!({ "now": 1_000 }))
        .await
        .unwrap();
    assert_eq!(
        due_ids(&later),
        vec!["e1"],
        "past backoff, the effect is due again"
    );
}
