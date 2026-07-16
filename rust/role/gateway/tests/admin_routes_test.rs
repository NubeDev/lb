//! Gateway parity + the server-deny-on-forged-call for the admin-crud surface (admin-crud +
//! admin-console scopes). Proves: (a) the destructive routes are reachable for an admin, (b) a
//! **forged** call by a non-admin is denied **server-side** (the UI cap-gate is NOT the boundary),
//! and (c) the `/login` route honors `user.disable` (disable bites login over the real transport).

mod common;

use axum::http::StatusCode;
use common::{bearer, delete_req, gateway, get_req, json_post, post_empty, token};
use lb_role_gateway::router;
use serde_json::json;
use tower::ServiceExt;

const ADMIN_CAPS: &[&str] = &[
    "mcp:user.manage:call",
    "mcp:user.disable:call",
    "mcp:teams.manage:call",
    "mcp:teams.list:call",
    "mcp:workspace.delete:call",
    "mcp:workspace.purge:call",
];

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn forged_admin_call_by_non_admin_is_denied_server_side() {
    let (gw, key) = gateway().await;
    // A valid session token, but holding NO admin caps — the UI would hide the controls; a forged
    // direct call must still be refused by the gateway (the boundary is the server).
    let tok = token(&key, "user:mallory", "acme", &["bus:chan/*:pub"]);

    for req in [
        delete_req("/admin/users/bob"),
        post_empty("/admin/users/bob/disable"),
        delete_req("/admin/teams/facilities"),
        delete_req("/teams/facilities/members/bob"),
        post_empty("/admin/workspaces/acme/archive"),
    ] {
        let resp = router(gw.clone()).oneshot(bearer(req, &tok)).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "forged admin call must be 403 server-side"
        );
    }
}

/// **The REAL member bundle cannot reach an admin route** (the 2026-07-16 wildcard leak).
///
/// `forged_admin_call_by_non_admin_is_denied_server_side` above mints a hand-picked cap set
/// (`bus:chan/*:pub`) — a principal no `/auth/login` ever issues. That is why it stayed green while
/// the wall was open: it never exercised what a real member actually holds. This test mints the
/// bundle `resolve_caps` really folds into a `role:member` token, so it tests the deployed grant.
///
/// The bug: `member_role_caps()` carried the broad author wildcards `mcp:*.list:call` /
/// `mcp:*.delete:call` / `mcp:*.create:call`, and the `*` spans the `<tool>` half of `<tool>.<verb>`
/// — so a plain member's caps SATISFIED `mcp:teams.list:call` at the wall. `GET /admin/teams`
/// returned 200 with the full team roster to `user:bob`, a plain member, on a live node. Nine other
/// admin-only caps (`roles.list`, `grants.list`, `invite.list`/`create`, `ext.list`,
/// `workspace.create`/`delete`, `series.delete`, `nav.delete`) were satisfied the same way.
///
/// `/admin/teams` is the canary. The invariant behind it — no member/viewer wildcard may span an
/// admin-only cap — is pinned exhaustively at the bundle in
/// `lb-host`'s `authz::builtin_roles::no_builtin_bundle_may_span_an_admin_only_cap`. This test is the
/// end-to-end half: it proves the wall itself, over the real router, for the real bundle.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_real_member_bundle_cannot_reach_an_admin_route() {
    let (gw, key) = gateway().await;
    let bundle = lb_host::member_role_caps();
    let member: Vec<&str> = bundle.iter().map(String::as_str).collect();
    let tok = token(&key, "user:bob", "acme", &member);

    // Reads: a member must not enumerate the workspace's people, teams, roles or grants.
    for uri in ["/admin/teams", "/admin/identities", "/admin/members"] {
        let resp = router(gw.clone())
            .oneshot(bearer(get_req(uri), &tok))
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "a plain member must NOT read {uri} — `GET /admin/teams` returned 200 with the full \
             roster to user:bob on a live node because his `mcp:*.list:call` satisfied \
             `mcp:teams.list:call`"
        );
    }

    // Writes: the destructive admin surface, driven by the real member bundle rather than a
    // hand-picked cap list.
    for req in [
        delete_req("/admin/users/bob"),
        post_empty("/admin/users/bob/disable"),
        delete_req("/admin/teams/facilities"),
        post_empty("/admin/workspaces/acme/archive"),
    ] {
        let resp = router(gw.clone()).oneshot(bearer(req, &tok)).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "the real member bundle must not reach a destructive admin route"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn admin_can_create_disable_and_delete_a_user_over_the_routes() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:alice", "acme", ADMIN_CAPS);

    // Create → 204.
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post("/admin/users", json!({ "user": "bob" })),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Disable → 204.
    let resp = router(gw.clone())
        .oneshot(bearer(post_empty("/admin/users/bob/disable"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Delete → 200 with the revoked count.
    let resp = router(gw)
        .oneshot(bearer(delete_req("/admin/users/bob"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn login_refuses_a_disabled_user_over_the_real_route() {
    let (gw, key) = gateway().await;
    let admin = token(&key, "user:alice", "acme", ADMIN_CAPS);

    // Seed + disable bob.
    router(gw.clone())
        .oneshot(bearer(
            json_post("/admin/users", json!({ "user": "bob" })),
            &admin,
        ))
        .await
        .unwrap();
    router(gw.clone())
        .oneshot(bearer(post_empty("/admin/users/bob/disable"), &admin))
        .await
        .unwrap();

    // bob logs in → 403 (disable bites the login path).
    let resp = router(gw.clone())
        .oneshot(json_post(
            "/login",
            json!({ "user": "bob", "workspace": "acme" }),
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "a disabled user cannot mint a session"
    );

    // A fresh user logging into a workspace they are NOT a member of (acme already has bob) is now
    // refused — global-identity decision #4 (zero memberships → no token). Auto-seed is preserved as
    // the first-login-to-an-EMPTY-workspace bootstrap (decision #3), shown next.
    let resp = router(gw.clone())
        .oneshot(json_post(
            "/login",
            json!({ "user": "newcomer", "workspace": "acme" }),
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "a non-member cannot mint into a workspace that already has members"
    );

    // The first login to a brand-new (empty) workspace bootstraps the requester (auto-seed preserved
    // as the first-member bootstrap, global-identity decision #3).
    let resp = router(gw)
        .oneshot(json_post(
            "/login",
            json!({ "user": "newcomer", "workspace": "brand-new-ws" }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ── roles editor route (admin-console redesign) — POST /admin/roles, the real role-define path ──

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn admin_can_define_and_list_a_role_and_no_widening_is_enforced() {
    let (gw, key) = gateway().await;
    // An admin who may define/list roles AND holds the cap they want to bundle.
    let admin = token(
        &key,
        "user:alice",
        "acme",
        &[
            "mcp:roles.define:call",
            "mcp:roles.list:call",
            "mcp:user.manage:call",
        ],
    );

    // Define a role bundling a cap the definer holds → 204.
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/admin/roles",
                json!({ "name": "user-admin", "caps": ["mcp:user.manage:call"] }),
            ),
            &admin,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT, "define a role → 204");

    // It comes back from GET /admin/roles.
    let resp = router(gw.clone())
        .oneshot(bearer(common::get_req("/admin/roles"), &admin))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // No-widening: bundling a cap the definer does NOT hold is refused server-side.
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/admin/roles",
                json!({ "name": "super", "caps": ["mcp:workspace.purge:call"] }),
            ),
            &admin,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "cannot bundle a cap you don't hold (no-widening)"
    );

    // A non-admin (no roles caps) is denied on both verbs — the forged-call boundary.
    let none = token(&key, "user:mallory", "acme", &["bus:chan/*:pub"]);
    for req in [
        common::get_req("/admin/roles"),
        json_post("/admin/roles", json!({ "name": "x", "caps": [] })),
    ] {
        let resp = router(gw.clone())
            .oneshot(bearer(req, &none))
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "roles route must deny non-admin"
        );
    }
}

// ── extension lifecycle routes (lifecycle-management scope) — the browser surface that was missing ──

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ext_routes_are_reachable_for_an_admin_and_deny_a_non_admin() {
    let (gw, key) = gateway().await;

    // An admin holding the ext caps: list is reachable (empty, no installs) and disable 204.
    let admin = token(
        &key,
        "user:alice",
        "acme",
        &[
            "mcp:ext.list:call",
            "mcp:ext.disable:call",
            "mcp:ext.uninstall:call",
        ],
    );
    let resp = router(gw.clone())
        .oneshot(bearer(common::get_req("/extensions"), &admin))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "ext_list reachable over the gateway"
    );
    // disable of an absent ext is idempotent success (no `unknown command` in the browser).
    let resp = router(gw.clone())
        .oneshot(bearer(post_empty("/extensions/hello/disable"), &admin))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // A non-admin (no ext caps) is denied server-side on every ext route — the forged-call boundary.
    let none = token(&key, "user:mallory", "acme", &["bus:chan/*:pub"]);
    for req in [
        common::get_req("/extensions"),
        post_empty("/extensions/hello/disable"),
        delete_req("/extensions/hello"),
    ] {
        let resp = router(gw.clone())
            .oneshot(bearer(req, &none))
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "ext route must deny non-admin"
        );
    }
}
