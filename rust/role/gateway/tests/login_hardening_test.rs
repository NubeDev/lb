//! Login-hardening scope — the headline regression + the credential-check seam, over the REAL
//! gateway + SurrealDB (no mocks, CLAUDE §9). Proves:
//!
//! (a) **The escalation is closed.** A plain member (`user:bob`, added to a workspace that already
//!     has an admin) logs in over `/login` and his admin calls — `members.add` (team member),
//!     `teams.manage` (create team), `grants.assign` (self-grant `workspace.delete`) — are all
//!     `403` server-side. Before this change every one was `204`: the member token carried the admin
//!     bundle. This is the exact live finding (`docs/debugging/auth-caps/member-token-carries-admin-caps.md`).
//! (b) **A member keeps member reach.** The same bob token still `200`s a member verb
//!     (`dashboard.list`) — we tightened admin, not the member surface.
//! (c) **First-principal bootstrap still yields a real admin.** The workspace's first login resolves
//!     to `role:workspace-admin` (seeded role record) and CAN run the admin verbs bob can't — proving
//!     the fix moved admin onto the role, not that it broke admin.
//! (d) **The credential check gates minting.** A `PasswordHash` gateway `401`s a login with a
//!     wrong/absent secret and `200`s the right one; the credential set in `acme` does not
//!     authenticate into `beta` (workspace isolation of the credential).

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::{bearer, gateway, gateway_on, json_post};
use lb_host::Node;
use lb_role_gateway::session::PasswordHash;
use lb_role_gateway::{router, Gateway};
use serde_json::json;
use tower::ServiceExt;

/// Log in over the real `/login` route (password-less dev check) and return the bearer token.
async fn login(gw: &Gateway, user: &str, ws: &str) -> String {
    let resp = router(gw.clone())
        .oneshot(json_post(
            "/login",
            json!({ "user": user, "workspace": ws }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "login {user}@{ws} ok");
    let reply: serde_json::Value = common::json_body(resp).await;
    reply["token"].as_str().unwrap().to_string()
}

// ── (a)+(b)+(c): the escalation, closed ─────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_member_login_cannot_run_admin_verbs_but_admin_bootstrap_still_can() {
    let (gw, _key) = gateway().await;

    // First login into an EMPTY workspace bootstraps `alice` as workspace-admin (decision #3): the
    // seeded `role:workspace-admin` record + the role grant resolve to the admin caps.
    let admin = login(&gw, "user:alice", "acme").await;

    // The admin adds `bob` as a plain member (so acme now has members → bob is not the bootstrap).
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post("/admin/members", json!({ "sub": "user:bob" })),
            &admin,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NO_CONTENT,
        "admin adds bob as a member"
    );

    // Bob logs in → a MEMBER token (trimmed base ∪ his resolved caps = only `role:member`).
    let bob = login(&gw, "user:bob", "acme").await;

    // (a) Every admin verb bob abused in the live session is now 403 server-side.
    // members.manage — add another member to the workspace (bob adding carol).
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post("/admin/members", json!({ "sub": "user:carol" })),
            &bob,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "member cannot add a member (members.manage) — was 204 before the fix"
    );

    // teams.manage — create a team.
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/admin/teams",
                json!({ "team": "facilities", "name": "Facilities" }),
            ),
            &bob,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "member cannot create a team (teams.manage) — was 204 before the fix"
    );

    // grants.assign — self-grant `mcp:workspace.delete:call` (the exact escalation bob pulled off).
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/admin/grants",
                json!({ "subject": "user:bob", "cap": "mcp:workspace.delete:call" }),
            ),
            &bob,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "member cannot self-grant workspace.delete (grants.assign) — was 204 before the fix"
    );

    // (b) Bob keeps member reach: a member verb still succeeds with the same token.
    let resp = router(gw.clone())
        .oneshot(bearer(common::get_req("/dashboards"), &bob))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "member reach intact — dashboard.list still 200s for bob"
    );

    // (c) The bootstrap admin CAN run the same verbs bob can't — admin power rides the role, works.
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/admin/teams",
                json!({ "team": "facilities", "name": "Facilities" }),
            ),
            &admin,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NO_CONTENT,
        "the workspace-admin (bootstrap) CAN create a team — admin moved onto the role, not broken"
    );
}

// ── deny-per-verb: a member cannot set another user's password ───────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_member_cannot_set_a_credential() {
    let (gw, _key) = gateway().await;
    // alice bootstraps as admin; bob is a plain member.
    let admin = login(&gw, "user:alice", "acme").await;
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post("/admin/members", json!({ "sub": "user:bob" })),
            &admin,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let bob = login(&gw, "user:bob", "acme").await;

    // bob (member) tries to set carol's password over the bridge → the `identity.manage` gate denies.
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/mcp/call",
                json!({ "tool": "identity.set_credential",
                        "args": { "user": "user:carol", "secret": "x", "ts": 1 } }),
            ),
            &bob,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "a member cannot set a credential (identity.manage) — deny-per-verb"
    );

    // The admin CAN (same call, admin token) → 200.
    let resp = router(gw)
        .oneshot(bearer(
            json_post(
                "/mcp/call",
                json!({ "tool": "identity.set_credential",
                        "args": { "user": "user:carol", "secret": "x", "ts": 1 } }),
            ),
            &admin,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "the workspace-admin CAN set a credential"
    );
}

// ── (d): the credential check gates minting (PasswordHash) ───────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn password_hash_gateway_401s_on_bad_or_absent_secret_and_isolates_by_workspace() {
    // A gateway wired with the REAL `PasswordHash` check (production posture), not `DevTrustAny`.
    let node = Arc::new(Node::boot_as(lb_host::Role::Hub).await.expect("node boots"));
    let key = lb_auth::SigningKey::generate();
    let dev_gw = gateway_on(node.clone(), &key); // password-less, to seed via the admin path
    let pw_gw = Gateway::new(node.clone(), key.clone(), common::NOW)
        .with_credential_check(Arc::new(PasswordHash));

    // Bootstrap alice as admin (password-less dev gateway), then set bob's password + add him.
    let admin = login(&dev_gw, "user:alice", "acme").await;
    let resp = router(dev_gw.clone())
        .oneshot(bearer(
            json_post("/admin/members", json!({ "sub": "user:bob" })),
            &admin,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    // Admin sets bob's credential over the MCP bridge (identity.set_credential, gated identity.manage).
    let resp = router(dev_gw.clone())
        .oneshot(bearer(
            json_post(
                "/mcp/call",
                json!({ "tool": "identity.set_credential",
                        "args": { "user": "user:bob", "secret": "hunter2", "ts": 1 } }),
            ),
            &admin,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "admin sets bob's password");

    // Wrong secret → 401, no token.
    let resp = router(pw_gw.clone())
        .oneshot(json_post(
            "/login",
            json!({ "user": "user:bob", "workspace": "acme", "secret": "wrong" }),
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "wrong secret → 401"
    );

    // Absent secret → 401 (a PasswordHash node refuses a password-less login).
    let resp = router(pw_gw.clone())
        .oneshot(json_post(
            "/login",
            json!({ "user": "user:bob", "workspace": "acme" }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "no secret → 401");

    // Right secret → 200 + a token.
    let resp = router(pw_gw.clone())
        .oneshot(json_post(
            "/login",
            json!({ "user": "user:bob", "workspace": "acme", "secret": "hunter2" }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "right secret → 200");

    // Workspace isolation of the credential: bob's `acme` password does not authenticate into `beta`.
    // (`beta` is empty, so absent-credential there → 401 under PasswordHash even with the right secret.)
    let resp = router(pw_gw)
        .oneshot(json_post(
            "/login",
            json!({ "user": "user:bob", "workspace": "beta", "secret": "hunter2" }),
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "a password set in acme does not authenticate into beta (credential ws-isolation)"
    );
}
