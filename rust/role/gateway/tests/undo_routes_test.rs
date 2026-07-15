//! The undo routes over the real gateway (undo-exposure scope) — `POST /undo`, `POST /redo`,
//! `GET /undo/history`, `GET /undo/history/{seq}/compensations`, end to end. Mirrors the host undo
//! tests at the transport boundary: the undo → redo round-trip over a real journal, the typed
//! `ok:false` refusals passed through as `200` data (the shell renders them — they are outcomes, not
//! errors), capability-deny per verb (opaque `403`), and two-session workspace isolation. The
//! gateway re-checks every gate server-side; the workspace + principal come from the token (§7).

mod common;

use axum::http::StatusCode;
use common::*;
use lb_host::{Node, Role as NodeRole};
use lb_role_gateway::router;
use lb_store::{read, write};
use lb_undo::{record_change, record_irreversible, Class, RecordChange, RecordIrreversible};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt; // for `oneshot`

/// The undo caps a dev member holds: their OWN stack (undo/redo), the history read, and the
/// compensations verb — plus the original tool's cap, which the host's no-escalation check demands.
const CAPS: &[&str] = &[
    "mcp:undo:call",
    "mcp:redo:call",
    "mcp:history.list:call",
    "mcp:history.compensations:call",
    "mcp:doc.rename:call",
    "store:doc:read",
    "store:doc:write",
];

/// Seed a real tracked rename into the journal (the capture path is internal to the host; the verbs
/// under test are undo/redo/history). The actor matches the token's `sub` — it is "their own" step.
async fn seed_rename(node: &Node, ws: &str, actor: &str) {
    write(&node.store, ws, "doc", "d1", &json!({"title": "draft"}))
        .await
        .unwrap();
    record_change(
        &node.store,
        RecordChange {
            ws,
            actor,
            surface: "",
            tool: "doc.rename",
            trace_id: "t",
            ts: 1,
            table: "doc",
            id: "d1",
            new_value: Some(&json!({"title": "v1"})),
            depth_cap: None,
        },
    )
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn undo_then_redo_round_trips_over_the_routes() {
    let (gw, key) = gateway().await;
    let node = gw.node.clone();
    seed_rename(&node, "acme", "user:ada").await;
    let tok = token(&key, "user:ada", "acme", CAPS);

    // POST /undo → the before-image is restored in the real store.
    let resp = router(gw.clone())
        .oneshot(bearer(post_empty("/undo"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = json_body(resp).await;
    assert_eq!(body["ok"], json!(true), "undo applied: {body}");
    assert_eq!(
        read(&node.store, "acme", "doc", "d1").await.unwrap(),
        Some(json!({"title": "draft"})),
        "undo restores the before-image through the route"
    );

    // POST /redo → forward again.
    let resp = router(gw)
        .oneshot(bearer(post_empty("/redo"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = json_body(resp).await;
    assert_eq!(body["ok"], json!(true), "redo applied: {body}");
    assert_eq!(
        read(&node.store, "acme", "doc", "d1").await.unwrap(),
        Some(json!({"title": "v1"})),
        "redo re-applies through the route"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn history_lists_the_callers_own_stack() {
    let (gw, key) = gateway().await;
    seed_rename(&gw.node.clone(), "acme", "user:ada").await;
    let tok = token(&key, "user:ada", "acme", CAPS);

    let resp = router(gw)
        .oneshot(bearer(get_req("/undo/history"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = json_body(resp).await;
    let items = body["items"].as_array().expect("items array: {body}");
    assert_eq!(items.len(), 1, "the seeded step is listed: {body}");
    assert_eq!(items[0]["tool"], "doc.rename");
    assert_eq!(items[0]["undoable"], json!(true));
}

/// An empty stack is a **normal outcome**, not an error: `200 {ok:false, reason:"empty"}` — the
/// shell greys the control rather than showing a failure.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_empty_stack_is_a_typed_refusal_not_an_error() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", CAPS);

    let resp = router(gw)
        .oneshot(bearer(post_empty("/undo"), &tok))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "a refusal is data, not an HTTP error"
    );
    let body: Value = json_body(resp).await;
    assert_eq!(body["ok"], json!(false));
    assert_eq!(body["reason"], "empty", "typed reason: {body}");
}

/// The conditional restore over the transport: an intervening writer makes the undo STALE, and the
/// route surfaces that as typed data while the collaborator's edit survives untouched.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_stale_undo_is_a_typed_refusal_and_clobbers_nothing() {
    let (gw, key) = gateway().await;
    let node = gw.node.clone();
    seed_rename(&node, "acme", "user:ada").await;
    // A collaborator writes after the tracked step.
    write(
        &node.store,
        "acme",
        "doc",
        "d1",
        &json!({"title": "theirs"}),
    )
    .await
    .unwrap();
    let tok = token(&key, "user:ada", "acme", CAPS);

    let resp = router(gw)
        .oneshot(bearer(post_empty("/undo"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = json_body(resp).await;
    assert_eq!(body["ok"], json!(false));
    assert_eq!(body["reason"], "stale", "typed reason: {body}");
    assert_eq!(
        read(&node.store, "acme", "doc", "d1").await.unwrap(),
        Some(json!({"title": "theirs"})),
        "a refused undo must never clobber the intervening write"
    );
}

/// A non-undoable step offers its compensation instead — the greyed row's "do this instead" affordance.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn compensations_surfaces_the_compensating_tool() {
    let (gw, key) = gateway().await;
    let seq = record_irreversible(
        &gw.node.store,
        RecordIrreversible {
            ws: "acme",
            actor: "user:ada",
            surface: "",
            tool: "workflow.open_pr",
            trace_id: "t",
            ts: 1,
            class: Class::Compensable {
                compensation_tool: "workflow.close_pr".into(),
            },
            group: None,
            depth_cap: None,
        },
    )
    .await
    .unwrap();
    let tok = token(&key, "user:ada", "acme", CAPS);

    let resp = router(gw)
        .oneshot(bearer(
            get_req(&format!("/undo/history/{seq}/compensations")),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = json_body(resp).await;
    assert_eq!(body["compensation_tool"], "workflow.close_pr", "{body}");
}

/// Capability-deny per verb (mandatory): each route is refused without its own grant, opaquely.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn each_verb_is_refused_without_its_grant() {
    let (gw, key) = gateway().await;
    seed_rename(&gw.node.clone(), "acme", "user:ada").await;

    // Holds every undo cap EXCEPT the one under test, so only the missing grant can explain the 403.
    let cases: [(&str, &str, bool); 4] = [
        ("mcp:undo:call", "/undo", true),
        ("mcp:redo:call", "/redo", true),
        ("mcp:history.list:call", "/undo/history", false),
        (
            "mcp:history.compensations:call",
            "/undo/history/1/compensations",
            false,
        ),
    ];
    for (missing, uri, is_post) in cases {
        let caps: Vec<&str> = CAPS.iter().copied().filter(|c| *c != missing).collect();
        let tok = token(&key, "user:ada", "acme", &caps);
        let req = if is_post {
            post_empty(uri)
        } else {
            get_req(uri)
        };
        let resp = router(gw.clone()).oneshot(bearer(req, &tok)).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "{uri} must be refused without {missing}"
        );
    }
}

/// The no-escalation rule at the transport: undo may not reach past the caps the caller already
/// holds — lacking the ORIGINAL tool's cap, the undo of that step is refused.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn undo_is_refused_without_the_original_tools_cap() {
    let (gw, key) = gateway().await;
    let node = gw.node.clone();
    seed_rename(&node, "acme", "user:ada").await;

    // Every undo cap, but NOT `mcp:doc.rename:call` — the step's own tool.
    let caps: Vec<&str> = CAPS
        .iter()
        .copied()
        .filter(|c| *c != "mcp:doc.rename:call")
        .collect();
    let tok = token(&key, "user:ada", "acme", &caps);

    let resp = router(gw)
        .oneshot(bearer(post_empty("/undo"), &tok))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "undo must not escalate past the caller's own caps"
    );
    assert_eq!(
        read(&node.store, "acme", "doc", "d1").await.unwrap(),
        Some(json!({"title": "v1"})),
        "the refused undo changed nothing"
    );
}

/// Workspace isolation (mandatory): two sessions on ONE node — ws-B can neither see nor undo ws-A's
/// journal. The wall comes from the token, never the request.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_b_cannot_see_or_undo_workspace_a_journal() {
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("boots"));
    let key = lb_auth::SigningKey::generate();
    seed_rename(&node, "acme", "user:ada").await;
    let gw_b = gateway_on(node.clone(), &key);
    let tok_b = token(&key, "user:ada", "other-co", CAPS);

    // ws-B's history is empty — ws-A's step is invisible.
    let resp = router(gw_b.clone())
        .oneshot(bearer(get_req("/undo/history"), &tok_b))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = json_body(resp).await;
    assert!(
        body["items"].as_array().unwrap().is_empty(),
        "ws-B must not see ws-A's journal: {body}"
    );

    // And a ws-B undo finds nothing — ws-A's record is untouched.
    let resp = router(gw_b)
        .oneshot(bearer(post_empty("/undo"), &tok_b))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = json_body(resp).await;
    assert_eq!(body["ok"], json!(false), "nothing to undo in ws-B: {body}");
    assert_eq!(
        read(&node.store, "acme", "doc", "d1").await.unwrap(),
        Some(json!({"title": "v1"})),
        "ws-A's record survives a ws-B undo attempt"
    );
}
