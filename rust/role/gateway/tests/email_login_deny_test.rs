//! Email-login scope — the DENY surface + workspace isolation for `/auth/*`, over the REAL gateway +
//! store. The security guardrail the scope names: a select-token is powerless everywhere except
//! `/auth/select`; a full token is refused at `/auth/select`; a select-token cannot `/auth/switch`;
//! and `/auth/switch` cannot reach a workspace the sub is not an effective member of.

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::{bearer, gateway_on, get_req, json_body, json_post, NOW};
use lb_auth::SigningKey;
use lb_host::Node;
use lb_role_gateway::session::GlobalPasswordHash;
use lb_role_gateway::{router, Gateway};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn real_gateway() -> (Gateway, Arc<Node>, SigningKey) {
    let node = Arc::new(Node::boot_as(lb_host::Role::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let gw = Gateway::new(node.clone(), key.clone(), NOW)
        .with_global_credential_check(Arc::new(GlobalPasswordHash));
    (gw, node, key)
}

async fn bootstrap_admin(gw: &Gateway, user: &str, ws: &str) -> String {
    let resp = router(gw.clone())
        .oneshot(json_post(
            "/login",
            json!({ "user": user, "workspace": ws }),
        ))
        .await
        .unwrap();
    let reply: Value = json_body(resp).await;
    reply["token"].as_str().unwrap().to_string()
}

async fn provision(gw: &Gateway, admin: &str, sub: &str, email: &str, pw: &str) {
    router(gw.clone())
        .oneshot(bearer(
            json_post("/admin/identities", json!({ "sub": sub, "email": email })),
            admin,
        ))
        .await
        .unwrap();
    router(gw.clone())
        .oneshot(bearer(
            json_post(
                &format!("/admin/identities/{sub}/password"),
                json!({ "secret": pw }),
            ),
            admin,
        ))
        .await
        .unwrap();
    router(gw.clone())
        .oneshot(bearer(
            json_post("/admin/members", json!({ "sub": sub })),
            admin,
        ))
        .await
        .unwrap();
}

/// Log in a multi-workspace user and return the select-token.
async fn select_token_for_two_ws(gw: &Gateway) -> String {
    let admin_a = bootstrap_admin(gw, "user:admin_a", "acme").await;
    let admin_b = bootstrap_admin(gw, "user:admin_b", "globex").await;
    provision(gw, &admin_a, "user:bob", "bob@x.com", "pw").await;
    // add bob to globex too
    router(gw.clone())
        .oneshot(bearer(
            json_post("/admin/members", json!({ "sub": "user:bob" })),
            &admin_b,
        ))
        .await
        .unwrap();
    let resp = router(gw.clone())
        .oneshot(json_post(
            "/auth/login",
            json!({ "email": "bob@x.com", "password": "pw" }),
        ))
        .await
        .unwrap();
    let body: Value = json_body(resp).await;
    body["select_token"].as_str().unwrap().to_string()
}

// ── A select-token is powerless on every NORMAL route ───────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_select_token_is_refused_by_normal_routes_and_verbs() {
    let (gw, node, key) = real_gateway().await;
    let select = select_token_for_two_ws(&gw).await;

    // An admin route (identity list) — empty ws + no caps → denied (401/403, never 200).
    let gw1 = gateway_on(node.clone(), &key);
    let resp = router(gw1)
        .oneshot(bearer(get_req("/admin/identities"), &select))
        .await
        .unwrap();
    assert_ne!(
        resp.status(),
        StatusCode::OK,
        "select-token must not read the identity directory"
    );

    // An MCP call — the capability gate refuses an empty-caps token.
    let gw2 = gateway_on(node.clone(), &key);
    let resp = router(gw2)
        .oneshot(bearer(
            json_post("/mcp/call", json!({ "tool": "identity.list", "input": {} })),
            &select,
        ))
        .await
        .unwrap();
    assert_ne!(
        resp.status(),
        StatusCode::OK,
        "select-token must not run an MCP verb"
    );

    // A data write route (channel post) — refused.
    let gw3 = gateway_on(node.clone(), &key);
    let resp = router(gw3)
        .oneshot(bearer(
            json_post("/channels/general/messages", json!({ "body": "hi" })),
            &select,
        ))
        .await
        .unwrap();
    assert_ne!(resp.status(), StatusCode::OK, "select-token cannot post");

    // But it IS accepted at /auth/select for a member workspace.
    let gw4 = gateway_on(node, &key);
    let resp = router(gw4)
        .oneshot(bearer(
            json_post("/auth/select", json!({ "workspace": "acme" })),
            &select,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "the select-token's ONE acceptor is /auth/select"
    );
}

// ── A full token is refused at /auth/select ─────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_full_token_is_refused_at_auth_select() {
    let (gw, node, key) = real_gateway().await;
    let admin = bootstrap_admin(&gw, "user:admin", "acme").await;
    provision(&gw, &admin, "user:ada", "ada@acme.com", "pw").await;
    let resp = router(gw.clone())
        .oneshot(json_post(
            "/auth/login",
            json!({ "email": "ada@acme.com", "password": "pw" }),
        ))
        .await
        .unwrap();
    let body: Value = json_body(resp).await;
    let full = body["token"].as_str().unwrap().to_string();

    let gw2 = gateway_on(node, &key);
    let resp = router(gw2)
        .oneshot(bearer(
            json_post("/auth/select", json!({ "workspace": "acme" })),
            &full,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "a full session token is not a select-token"
    );
}

// ── /auth/switch: isolation + freshness ─────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn switch_to_a_non_member_workspace_is_forbidden() {
    let (gw, node, key) = real_gateway().await;
    // Ada is a member of acme only; a separate admin owns globex.
    let admin_a = bootstrap_admin(&gw, "user:admin_a", "acme").await;
    let _admin_b = bootstrap_admin(&gw, "user:admin_b", "globex").await;
    provision(&gw, &admin_a, "user:ada", "ada@acme.com", "pw").await;

    let resp = router(gw.clone())
        .oneshot(json_post(
            "/auth/login",
            json!({ "email": "ada@acme.com", "password": "pw" }),
        ))
        .await
        .unwrap();
    let body: Value = json_body(resp).await;
    let token = body["token"].as_str().unwrap().to_string();

    // Ada tries to switch into globex — she is not a member → 403.
    let gw2 = gateway_on(node.clone(), &key);
    let resp = router(gw2)
        .oneshot(bearer(
            json_post("/auth/switch", json!({ "workspace": "globex" })),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "switch to a non-member workspace is refused server-side"
    );

    // But switching back into her own workspace works (no password).
    let gw3 = gateway_on(node, &key);
    let resp = router(gw3)
        .oneshot(bearer(
            json_post("/auth/switch", json!({ "workspace": "acme" })),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "switch into a member ws re-mints"
    );
}
