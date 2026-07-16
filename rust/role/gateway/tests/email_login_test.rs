//! Email-login scope — the Slack-style front door (`/auth/login|select|switch|password`) over the
//! REAL gateway + SurrealDB (no mocks, argon2 real; CLAUDE §9). Proves the scope's testing plan:
//! the 0/1/N branch, the select-token deny surface, workspace isolation of the switch, timing-uniform
//! credential failure, email uniqueness + case-insensitivity, and the self-service change.
//!
//! Seeding uses the REAL write path: an admin is bootstrapped via `/login` into an empty workspace,
//! then provisions global identities (`POST /admin/identities` with email), sets their global password
//! (`POST /admin/identities/{sub}/password`), and adds memberships (`POST /admin/members`) — exactly
//! the operator provisioning path the scope names. `/auth/*` is then driven end to end.

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::{bearer, gateway_on, json_body, json_post, NOW};
use lb_auth::SigningKey;
use lb_host::Node;
use lb_role_gateway::session::GlobalPasswordHash;
use lb_role_gateway::{router, Gateway};
use serde_json::{json, Value};
use tower::ServiceExt;

/// A gateway whose GLOBAL credential check is the REAL argon2 `GlobalPasswordHash` (so `/auth/login`
/// demands a correct password), on a fresh node. Returns the gateway + node so tests can build a
/// second co-node gateway for isolation.
async fn real_gateway() -> (Gateway, Arc<Node>, SigningKey) {
    let node = Arc::new(Node::boot_as(lb_host::Role::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let gw = Gateway::new(node.clone(), key.clone(), NOW)
        .with_global_credential_check(Arc::new(GlobalPasswordHash));
    (gw, node, key)
}

/// Bootstrap an admin: first `/login` into an empty workspace makes the requester workspace-admin.
async fn bootstrap_admin(gw: &Gateway, user: &str, ws: &str) -> String {
    let resp = router(gw.clone())
        .oneshot(json_post(
            "/login",
            json!({ "user": user, "workspace": ws }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "bootstrap {user}@{ws}");
    let reply: Value = json_body(resp).await;
    reply["token"].as_str().unwrap().to_string()
}

/// Provision a global identity with an email as `admin`.
async fn create_identity(gw: &Gateway, admin: &str, sub: &str, email: &str) -> StatusCode {
    router(gw.clone())
        .oneshot(bearer(
            json_post("/admin/identities", json!({ "sub": sub, "email": email })),
            admin,
        ))
        .await
        .unwrap()
        .status()
}

/// Set a global password as `admin`.
async fn set_password(gw: &Gateway, admin: &str, sub: &str, secret: &str) -> StatusCode {
    router(gw.clone())
        .oneshot(bearer(
            json_post(
                &format!("/admin/identities/{sub}/password"),
                json!({ "secret": secret }),
            ),
            admin,
        ))
        .await
        .unwrap()
        .status()
}

/// Add `sub` to `ws` as `admin` (admin must hold a token for `ws`).
async fn add_member(gw: &Gateway, admin: &str, sub: &str) -> StatusCode {
    router(gw.clone())
        .oneshot(bearer(
            json_post("/admin/members", json!({ "sub": sub })),
            admin,
        ))
        .await
        .unwrap()
        .status()
}

/// `POST /auth/login` — returns (status, body). An error response (403/401) carries a plain-text
/// body, not JSON, so decode leniently to `Value::Null` there.
async fn auth_login(gw: &Gateway, email: &str, password: &str) -> (StatusCode, Value) {
    let resp = router(gw.clone())
        .oneshot(json_post(
            "/auth/login",
            json!({ "email": email, "password": password }),
        ))
        .await
        .unwrap();
    let status = resp.status();
    let text = common::body_text(resp).await;
    let body = serde_json::from_str(&text).unwrap_or(Value::Null);
    (status, body)
}

// ── The 1-branch: single-workspace auto-skip ────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn one_workspace_login_returns_the_full_token_and_no_select_token() {
    let (gw, _node, _key) = real_gateway().await;
    let admin = bootstrap_admin(&gw, "user:admin", "acme").await;
    assert_eq!(
        create_identity(&gw, &admin, "user:ada", "ada@acme.com").await,
        StatusCode::OK
    );
    assert_eq!(
        set_password(&gw, &admin, "user:ada", "hunter2").await,
        StatusCode::OK
    );
    assert_eq!(
        add_member(&gw, &admin, "user:ada").await,
        StatusCode::NO_CONTENT
    );

    let (status, body) = auth_login(&gw, "ada@acme.com", "hunter2").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["token"].is_string(), "1-branch mints a full token");
    assert!(
        body["select_token"].is_null(),
        "no select-token in the 1-branch"
    );
    assert_eq!(body["principal"], "user:ada");
    assert_eq!(body["workspace"], "acme");
    assert_eq!(body["workspaces"][0]["ws"], "acme");
    assert_eq!(body["workspaces"][0]["name"], "acme");
}

// ── The N-branch: select-token + roster, then /auth/select ──────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn multi_workspace_login_returns_a_select_token_and_select_mints() {
    let (gw, node, key) = real_gateway().await;
    // Two workspaces, each bootstrapped by its own admin.
    let admin_a = bootstrap_admin(&gw, "user:admin_a", "acme").await;
    let admin_b = bootstrap_admin(&gw, "user:admin_b", "globex").await;
    assert_eq!(
        create_identity(&gw, &admin_a, "user:bob", "bob@x.com").await,
        StatusCode::OK
    );
    assert_eq!(
        set_password(&gw, &admin_a, "user:bob", "s3cret").await,
        StatusCode::OK
    );
    // Bob is a member of BOTH (admin_a adds in acme, admin_b in globex).
    assert_eq!(
        add_member(&gw, &admin_a, "user:bob").await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        add_member(&gw, &admin_b, "user:bob").await,
        StatusCode::NO_CONTENT
    );

    let (status, body) = auth_login(&gw, "bob@x.com", "s3cret").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["token"].is_null(), "N-branch withholds the full token");
    let select = body["select_token"].as_str().expect("select-token present");
    let roster: Vec<String> = body["workspaces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["ws"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(roster, vec!["acme", "globex"], "roster lists both, sorted");

    // Pick globex with the select-token → full token.
    let gw2 = gateway_on(node, &key);
    let resp = router(gw2)
        .oneshot(bearer(
            json_post("/auth/select", json!({ "workspace": "globex" })),
            select,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let picked: Value = json_body(resp).await;
    assert!(picked["token"].is_string(), "select mints the full token");
    assert_eq!(picked["workspace"], "globex");
    assert_eq!(picked["principal"], "user:bob");
}

// ── The 0-branch: authenticated but no membership ───────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn zero_workspaces_is_forbidden_with_no_token() {
    let (gw, _node, _key) = real_gateway().await;
    let admin = bootstrap_admin(&gw, "user:admin", "acme").await;
    // Carol has an identity + password but NO membership anywhere.
    assert_eq!(
        create_identity(&gw, &admin, "user:carol", "carol@x.com").await,
        StatusCode::OK
    );
    assert_eq!(
        set_password(&gw, &admin, "user:carol", "pw").await,
        StatusCode::OK
    );

    let (status, body) = auth_login(&gw, "carol@x.com", "pw").await;
    assert_eq!(status, StatusCode::FORBIDDEN, "no membership → 403");
    assert!(body["token"].is_null());
    assert!(body["select_token"].is_null());
}

// ── Uniform credential failure: unknown email and wrong password have the SAME 401 body ─────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn unknown_email_and_wrong_password_return_the_same_401() {
    let (gw, _node, _key) = real_gateway().await;
    let admin = bootstrap_admin(&gw, "user:admin", "acme").await;
    create_identity(&gw, &admin, "user:ada", "ada@acme.com").await;
    set_password(&gw, &admin, "user:ada", "right").await;
    add_member(&gw, &admin, "user:ada").await;

    // Wrong password for a KNOWN email.
    let resp_wrong = router(gw.clone())
        .oneshot(json_post(
            "/auth/login",
            json!({ "email": "ada@acme.com", "password": "wrong" }),
        ))
        .await
        .unwrap();
    let status_wrong = resp_wrong.status();
    let body_wrong = common::body_text(resp_wrong).await;

    // UNKNOWN email entirely.
    let resp_unknown = router(gw.clone())
        .oneshot(json_post(
            "/auth/login",
            json!({ "email": "nobody@x.com", "password": "whatever" }),
        ))
        .await
        .unwrap();
    let status_unknown = resp_unknown.status();
    let body_unknown = common::body_text(resp_unknown).await;

    assert_eq!(status_wrong, StatusCode::UNAUTHORIZED);
    assert_eq!(status_unknown, StatusCode::UNAUTHORIZED);
    assert_eq!(
        body_wrong, body_unknown,
        "wrong-password and unknown-email must return the IDENTICAL body (no enumeration oracle)"
    );
    assert_eq!(body_wrong, "invalid credentials");
}

// ── Email uniqueness + case-insensitivity ───────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_duplicate_email_is_refused_case_insensitively_and_lookup_is_case_insensitive() {
    let (gw, _node, _key) = real_gateway().await;
    let admin = bootstrap_admin(&gw, "user:admin", "acme").await;
    assert_eq!(
        create_identity(&gw, &admin, "user:ada", "Ada@Acme.com").await,
        StatusCode::OK
    );
    // A DIFFERENT identity claiming the same email (different case) is a 409.
    assert_eq!(
        create_identity(&gw, &admin, "user:ada2", "ada@acme.COM").await,
        StatusCode::CONFLICT,
        "case-folded duplicate email is refused"
    );

    // Lookup is case-insensitive: login with a differently-cased email finds ada.
    set_password(&gw, &admin, "user:ada", "pw").await;
    add_member(&gw, &admin, "user:ada").await;
    let (status, body) = auth_login(&gw, "ADA@acme.com", "pw").await;
    assert_eq!(status, StatusCode::OK, "case-insensitive email lookup");
    assert_eq!(body["principal"], "user:ada");
}

// ── Self-service password change ────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn self_service_change_requires_the_old_password_and_rotates() {
    let (gw, node, key) = real_gateway().await;
    let admin = bootstrap_admin(&gw, "user:admin", "acme").await;
    create_identity(&gw, &admin, "user:ada", "ada@acme.com").await;
    set_password(&gw, &admin, "user:ada", "old-pw").await;
    add_member(&gw, &admin, "user:ada").await;

    // Log in to get a full token.
    let (_s, body) = auth_login(&gw, "ada@acme.com", "old-pw").await;
    let token = body["token"].as_str().unwrap().to_string();

    // Wrong old password → 401, no change.
    let gw2 = gateway_on(node.clone(), &key);
    let resp = router(gw2)
        .oneshot(bearer(
            json_post("/auth/password", json!({ "old": "nope", "new": "new-pw" })),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "wrong old → 401");

    // Correct old → rotates.
    let gw3 = gateway_on(node.clone(), &key);
    let resp = router(gw3)
        .oneshot(bearer(
            json_post(
                "/auth/password",
                json!({ "old": "old-pw", "new": "new-pw" }),
            ),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "correct old → rotate");

    // Old password no longer logs in; new one does.
    assert_eq!(
        auth_login(&gw, "ada@acme.com", "old-pw").await.0,
        StatusCode::UNAUTHORIZED,
        "old password no longer works"
    );
    assert_eq!(
        auth_login(&gw, "ada@acme.com", "new-pw").await.0,
        StatusCode::OK,
        "new password works"
    );
}
