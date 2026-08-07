//! Login-hardening scope — the headline regression + the credential-check seam, over the REAL
//! gateway + SurrealDB (no mocks, CLAUDE §9). Proves:
//!
//! (a) **The escalation is closed.** A plain member (`user:bob`, added to a workspace that already
//!     has an admin) gets a real session and his admin calls — `members.add` (team member),
//!     `teams.manage` (create team), `grants.assign` (self-grant `workspace.delete`) — are all
//!     `403` server-side. Before this change every one was `204`: the member token carried the admin
//!     bundle. This is the exact live finding (`docs/debugging/auth-caps/member-token-carries-admin-caps.md`).
//! (b) **A member keeps member reach.** The same bob token still `200`s a member verb
//!     (`dashboard.list`) — we tightened admin, not the member surface.
//! (c) **The provisioned first admin is a real admin.** The operator-provisioned `workspace-admin`
//!     (`common::bootstrap`, the explicit first-admin path that replaced the deleted `/login`
//!     empty-workspace self-bootstrap) CAN run the admin verbs bob can't — proving the fix moved
//!     admin onto the role, not that it broke admin.
//!
//! The credential-gate case that used to live here (d) is GONE with the route it tested: `POST /login`
//! and its per-`(ws, user)` `CredentialCheck` were deleted in the pre-production legacy sweep. The
//! equivalent — wrong/absent password `401`s, right password `200`s — is pinned against the ONE
//! surviving door in `email_login_test.rs` / `email_login_deny_test.rs`.

mod common;

use axum::http::StatusCode;
use common::bootstrap::{provision_admin, session_token};
use common::{bearer, gateway, json_post};
use lb_role_gateway::router;
use serde_json::json;
use tower::ServiceExt;

// ── (a)+(b)+(c): the escalation, closed ─────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_member_login_cannot_run_admin_verbs_but_admin_bootstrap_still_can() {
    let (gw, _key) = gateway().await;

    // The operator provisions `alice` as the workspace-admin (the explicit first-admin path): the
    // seeded `role:workspace-admin` record + the role grant resolve to the admin caps.
    let admin = provision_admin(&gw, "user:alice", "nube").await;

    // The admin adds `bob` as a plain member (so nube now has members → bob is not the bootstrap).
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

    // Bob's session → a MEMBER token (viewer floor ∪ his resolved caps = only `role:member`).
    let bob = session_token(&gw, "user:bob", "nube").await;

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

    // (c) The provisioned admin CAN run the same verbs bob can't — admin power rides the role, works.
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
        "the provisioned workspace-admin CAN create a team — admin moved onto the role, not broken"
    );
}

// ── deny-per-verb: a member cannot set another user's password ───────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_member_cannot_set_a_credential() {
    let (gw, _key) = gateway().await;
    // alice is the provisioned admin; bob is a plain member.
    let admin = provision_admin(&gw, "user:alice", "nube").await;
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post("/admin/members", json!({ "sub": "user:bob" })),
            &admin,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let bob = session_token(&gw, "user:bob", "nube").await;

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
