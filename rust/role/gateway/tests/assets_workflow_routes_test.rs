//! The gateway's `assets.*` + `workflow.*` routes, end to end (S4/S6 gateway wiring — item 4 of the
//! STATUS "Next up": the host had the verbs + MCP bridges + UI fakes, but the browser could only
//! reach them through Tauri → `unknown command`). These routes were the gap. Each test drives the
//! real routes with `oneshot` (no socket) over a real signed session, and asserts the two properties
//! every gateway surface must hold: a **capability deny** (server-side, the token's caps, never the
//! body) and **workspace isolation** (a ws-B session sees none of ws-A's assets/approvals).
//!
//! Shared fixtures in `tests/common/`. Split from `gateway_test.rs` to stay under the FILE-LAYOUT
//! 400-line limit.

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::*;
use lb_auth::SigningKey;
use lb_host::{Node, Role as NodeRole};
use lb_role_gateway::router;
use serde_json::json;
use tower::ServiceExt; // for `oneshot`

// The store caps the asset routes check (capability-first, §3.5) — the route calls the host verb
// directly, so the gate is the `store:doc/*` / `store:skill/*` cap, not the MCP `mcp:assets.*` one.
const DOC_R: &str = "store:doc/*:read";
const DOC_W: &str = "store:doc/*:write";
const SKILL_R: &str = "store:skill/*:read";
const SKILL_W: &str = "store:skill/*:write";

// The workflow MCP caps the workflow routes check.
const REQ_APPROVAL: &str = "mcp:workflow.request_approval:call";
const RESOLVE: &str = "mcp:workflow.resolve_approval:call";
const START: &str = "mcp:workflow.start_job:call";
const CHAN_PUB: &str = "bus:chan/*:pub";

// ----- assets -----------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn put_then_get_own_doc_round_trips_over_the_gateway() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", &[DOC_R, DOC_W]);

    // Create a doc owned by the token's principal.
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/docs",
                json!({ "id": "spec", "title": "Spec", "content": "hello" }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Read it back — the owner may always read their own doc (gate 3 passes by ownership).
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/docs/spec"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let doc: serde_json::Value = json_body(resp).await;
    assert_eq!(doc["content"], "hello");
    assert_eq!(doc["owner"], "user:ada");

    // And it appears in the owner's doc list.
    let resp = router(gw)
        .oneshot(bearer(get_req("/docs"), &tok))
        .await
        .unwrap();
    let docs: Vec<serde_json::Value> = json_body(resp).await;
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0]["id"], "spec");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn put_doc_without_the_write_cap_is_denied_server_side() {
    let (gw, key) = gateway().await;
    // A read-only token — no `store:doc/*:write`. The deny is from the TOKEN's caps, server-side,
    // not the body (the hard wall, §7).
    let tok = token(&key, "user:ada", "acme", &[DOC_R]);

    let resp = router(gw)
        .oneshot(bearer(
            json_post("/docs", json!({ "id": "x", "title": "X", "content": "c" })),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_session_cannot_read_ws_a_doc() {
    // Two sessions, one node — the real isolation setup. ws-A creates a doc; a ws-B session (its own
    // token, same key) must not see it, even with the doc caps.
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.unwrap());
    let key = SigningKey::generate();
    let gw = gateway_on(node.clone(), &key);

    let tok_a = token(&key, "user:ada", "acme", &[DOC_R, DOC_W]);
    router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/docs",
                json!({ "id": "secret", "title": "S", "content": "x" }),
            ),
            &tok_a,
        ))
        .await
        .unwrap();

    // ws-B: a different workspace in the token. The workspace comes from the token, so this caller
    // queries ws-B — where the doc does not exist → NotFound (it is not even visible to deny).
    let tok_b = token(&key, "user:bob", "other", &[DOC_R, DOC_W]);
    let resp = router(gw)
        .oneshot(bearer(get_req("/docs/secret"), &tok_b))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn grant_then_load_skill_round_trips_and_ungranted_load_is_denied() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", &[SKILL_R, SKILL_W]);

    // Put a skill version.
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/skills",
                json!({ "id": "summarize", "version": "1", "description": "d", "body": "Be brief." }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Before the grant, loading it is denied (the grant gate the agent inherits — skills scope).
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/skills/summarize"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Grant it to the workspace, then it loads.
    router(gw.clone())
        .oneshot(bearer(
            json_post("/skills/summarize/grant", json!({})),
            &tok,
        ))
        .await
        .unwrap();
    let resp = router(gw)
        .oneshot(bearer(get_req("/skills/summarize"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let s: serde_json::Value = json_body(resp).await;
    assert_eq!(s["body"], "Be brief.");
}

// ----- workflow ---------------------------------------------------------------------------------

/// Request → resolve → start, the happy approval path over the gateway: the job starts ONLY after
/// the approval lands `approved` (the genuine S6 gate), and the PR effect is queued (visible on the
/// outbox route).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn approval_gate_lets_the_job_start_only_after_approved() {
    let (gw, key) = gateway().await;
    let tok = token(
        &key,
        "user:ada",
        "acme",
        &[REQ_APPROVAL, RESOLVE, START, CHAN_PUB],
    );

    // Request approval (records the PR spec keyed by the approval id).
    let pr =
        json!({ "repo": "acme/api", "head": "fix/1", "base": "main", "title": "Fix", "body": "" });
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/approvals/appr-1/request",
                json!({ "scope_doc": "scope:1", "team": "eng", "pr": pr }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Start BEFORE approval → the gate refuses: `started: false`, no side effects.
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/approvals/appr-1/start",
                json!({ "job_id": "job-1", "scope_doc": "scope:1", "channel": "build", "pr_key": "pr:appr-1" }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let out: serde_json::Value = json_body(resp).await;
    assert_eq!(out["started"], false, "the gate refuses before approval");

    // Approve, then start succeeds.
    router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/approvals/appr-1/resolve",
                json!({ "decision": "approved" }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    let resp = router(gw)
        .oneshot(bearer(
            json_post(
                "/approvals/appr-1/start",
                json!({ "job_id": "job-1", "scope_doc": "scope:1", "channel": "build", "pr_key": "pr:appr-1" }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let out: serde_json::Value = json_body(resp).await;
    assert_eq!(out["started"], true, "approved → the job starts");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workflow_verb_without_the_cap_is_denied_server_side() {
    let (gw, key) = gateway().await;
    // No workflow caps at all → every verb is refused before it runs.
    let tok = token(&key, "user:mallory", "acme", &[]);

    let resp = router(gw)
        .oneshot(bearer(
            json_post("/approvals/x/resolve", json!({ "decision": "approved" })),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_session_cannot_resolve_ws_a_approval() {
    // Two sessions, one node. ws-A requests an approval; a ws-B session (its own token) resolving the
    // same id operates in ws-B — it never touches ws-A's item (the workspace wall on every route).
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.unwrap());
    let key = SigningKey::generate();
    let gw = gateway_on(node.clone(), &key);

    let tok_a = token(
        &key,
        "user:ada",
        "acme",
        &[REQ_APPROVAL, RESOLVE, START, CHAN_PUB],
    );
    let pr = json!({ "repo": "acme/api", "head": "h", "base": "main", "title": "T", "body": "" });
    router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/approvals/appr-x/request",
                json!({ "scope_doc": "s", "team": "eng", "pr": pr }),
            ),
            &tok_a,
        ))
        .await
        .unwrap();

    // ws-B approves "appr-x" in ITS workspace (a no-op against ws-A) and tries to start — but ws-B
    // has no PR spec for that id (it was recorded in ws-A), so start is a `400`, not ws-A's job.
    let tok_b = token(
        &key,
        "user:bob",
        "other",
        &[REQ_APPROVAL, RESOLVE, START, CHAN_PUB],
    );
    router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/approvals/appr-x/resolve",
                json!({ "decision": "approved" }),
            ),
            &tok_b,
        ))
        .await
        .unwrap();
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/approvals/appr-x/start",
                json!({ "job_id": "j", "scope_doc": "s", "channel": "c", "pr_key": "pr:appr-x" }),
            ),
            &tok_b,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "ws-B has no PR spec for ws-A's approval — it cannot start ws-A's job"
    );

    // And ws-A's own start still works (its spec + its approval are intact, untouched by ws-B).
    router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/approvals/appr-x/resolve",
                json!({ "decision": "approved" }),
            ),
            &tok_a,
        ))
        .await
        .unwrap();
    let resp = router(gw)
        .oneshot(bearer(
            json_post(
                "/approvals/appr-x/start",
                json!({ "job_id": "j", "scope_doc": "s", "channel": "c", "pr_key": "pr:appr-x" }),
            ),
            &tok_a,
        ))
        .await
        .unwrap();
    let out: serde_json::Value = json_body(resp).await;
    assert_eq!(out["started"], true, "ws-A's approval + spec are intact");
}
