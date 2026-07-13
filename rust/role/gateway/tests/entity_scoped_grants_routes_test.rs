//! Gateway passthrough for **entity-scoped grants** (entity-scoped-grants scope, review fix):
//! `POST /admin/grants` (+ `/admin/grants/revoke`) accept an optional additive `scope` selector —
//! previously the REST path hardcoded `Scope::All`, so the admin console could never manage a
//! scoped grant over HTTP. Real node, real store, real gateway — no mocks. Includes the mandatory
//! capability-deny and workspace-isolation tests.

mod common;

use axum::http::StatusCode;
use common::{bearer, gateway, json_post, token, NOW};
use lb_auth::verify;
use lb_host::{grants_list_scoped, Scope, Subject};
use lb_role_gateway::router;
use serde_json::json;
use tower::ServiceExt;

const ADMIN: &[&str] = &[
    "mcp:grants.assign:call",
    "mcp:grants.list:call",
    // held so no-widening is satisfied when granting it to ana.
    "mcp:hvac.setpoint:call",
];

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn assign_route_persists_the_scope_selector() {
    let (gw, key) = gateway().await;
    let admin_tok = token(&key, "user:alice", "acme", ADMIN);

    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/admin/grants",
                json!({
                    "subject": "user:ana",
                    "cap": "mcp:hvac.setpoint:call",
                    "scope": { "kind": "ids", "table": "child", "ids": ["leo"] },
                }),
            ),
            &admin_tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // The stored grant carries the selector — not Scope::All.
    let admin = verify(&key, &admin_tok, NOW).unwrap();
    let grants = grants_list_scoped(&gw.node.store, &admin, "acme", &Subject::User("ana".into()))
        .await
        .unwrap();
    assert_eq!(grants.len(), 1);
    assert_eq!(
        grants[0].scope,
        Scope::Ids {
            table: "child".into(),
            ids: vec!["leo".into()],
        }
    );

    // Revoke with the same selector removes it (the scoped row, not a phantom All row).
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/admin/grants/revoke",
                json!({
                    "subject": "user:ana",
                    "cap": "mcp:hvac.setpoint:call",
                    "scope": { "kind": "ids", "table": "child", "ids": ["leo"] },
                }),
            ),
            &admin_tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let grants = grants_list_scoped(&gw.node.store, &admin, "acme", &Subject::User("ana".into()))
        .await
        .unwrap();
    assert!(grants.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn assign_route_without_scope_defaults_to_all() {
    // Additive: the pre-scope body shape still works and means Scope::All (zero migration).
    let (gw, key) = gateway().await;
    let admin_tok = token(&key, "user:alice", "acme", ADMIN);

    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/admin/grants",
                json!({ "subject": "user:ana", "cap": "mcp:hvac.setpoint:call" }),
            ),
            &admin_tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let admin = verify(&key, &admin_tok, NOW).unwrap();
    let grants = grants_list_scoped(&gw.node.store, &admin, "acme", &Subject::User("ana".into()))
        .await
        .unwrap();
    assert_eq!(grants.len(), 1);
    assert_eq!(grants[0].scope, Scope::All);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn malformed_scope_in_body_is_rejected_not_widened() {
    // A malformed selector must fail the request — never silently deserialize to All.
    let (gw, key) = gateway().await;
    let admin_tok = token(&key, "user:alice", "acme", ADMIN);

    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/admin/grants",
                json!({
                    "subject": "user:ana",
                    "cap": "mcp:hvac.setpoint:call",
                    "scope": { "kind": "idz", "table": "child" },
                }),
            ),
            &admin_tok,
        ))
        .await
        .unwrap();
    assert!(
        resp.status().is_client_error(),
        "malformed scope must be a 4xx, got {}",
        resp.status()
    );

    // Nothing was written.
    let admin = verify(&key, &admin_tok, NOW).unwrap();
    let grants = grants_list_scoped(&gw.node.store, &admin, "acme", &Subject::User("ana".into()))
        .await
        .unwrap();
    assert!(grants.is_empty());
}

// ── Mandatory: capability deny ────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn scoped_assign_denied_without_grants_cap() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:mallory", "acme", &["mcp:hvac.setpoint:call"]);

    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/admin/grants",
                json!({
                    "subject": "user:mallory",
                    "cap": "mcp:hvac.setpoint:call",
                    "scope": { "kind": "ids", "table": "child", "ids": ["leo"] },
                }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// ── Mandatory: workspace isolation ────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn scoped_grant_stays_inside_its_workspace() {
    // A scoped grant written through the acme session never appears under globex — the ws in the
    // grant key comes from the token, and resolution reads only the caller's namespace.
    let (gw, key) = gateway().await;
    let acme_tok = token(&key, "user:alice", "acme", ADMIN);
    router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/admin/grants",
                json!({
                    "subject": "user:ana",
                    "cap": "mcp:hvac.setpoint:call",
                    "scope": { "kind": "ids", "table": "child", "ids": ["leo"] },
                }),
            ),
            &acme_tok,
        ))
        .await
        .unwrap();

    let globex_tok = token(&key, "user:alice", "globex", ADMIN);
    let globex_admin = verify(&key, &globex_tok, NOW).unwrap();
    let grants = grants_list_scoped(
        &gw.node.store,
        &globex_admin,
        "globex",
        &Subject::User("ana".into()),
    )
    .await
    .unwrap();
    assert!(grants.is_empty(), "grant leaked across the workspace wall");
}
