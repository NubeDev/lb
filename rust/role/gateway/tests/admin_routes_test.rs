//! Gateway parity + the server-deny-on-forged-call for the admin-crud surface (admin-crud +
//! admin-console scopes). Proves: (a) the destructive routes are reachable for an admin, and (b) a
//! **forged** call by a non-admin is denied **server-side** (the UI cap-gate is NOT the boundary).
//!
//! The `/admin/users*` routes and the `/login`-honors-`user.disable` case are GONE — the legacy `user`
//! record and `POST /login` were deleted in the pre-production legacy sweep (email-login scope). The
//! roster lives at `/admin/members` (`admin_members`/`identity_routes_test`), the login path at
//! `/auth/*` (`email_login_test`), and there is no per-workspace disable at all.

mod common;

use axum::http::StatusCode;
use common::{bearer, delete_req, gateway, get_req, json_post, post_empty, token};
use lb_role_gateway::router;
use serde_json::json;
use tower::ServiceExt;

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn forged_admin_call_by_non_admin_is_denied_server_side() {
    let (gw, key) = gateway().await;
    // A valid session token, but holding NO admin caps — the UI would hide the controls; a forged
    // direct call must still be refused by the gateway (the boundary is the server).
    let tok = token(&key, "user:mallory", "nube", &["bus:chan/*:pub"]);

    for req in [
        delete_req("/admin/members/user:bob"),
        delete_req("/admin/teams/facilities"),
        delete_req("/teams/facilities/members/bob"),
        post_empty("/admin/workspaces/nube/archive"),
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
    let tok = token(&key, "user:bob", "nube", &member);

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
        delete_req("/admin/members/user:bob"),
        delete_req("/admin/teams/facilities"),
        post_empty("/admin/workspaces/nube/archive"),
    ] {
        let resp = router(gw.clone()).oneshot(bearer(req, &tok)).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "the real member bundle must not reach a destructive admin route"
        );
    }
}

// ── roles editor route (admin-console redesign) — POST /admin/roles, the real role-define path ──

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn admin_can_define_and_list_a_role_and_no_widening_is_enforced() {
    let (gw, key) = gateway().await;
    // An admin who may define/list roles AND holds the cap they want to bundle.
    let admin = token(
        &key,
        "user:alice",
        "nube",
        &[
            "mcp:roles.define:call",
            "mcp:roles.list:call",
            "mcp:members.manage:call",
        ],
    );

    // Define a role bundling a cap the definer holds → 204.
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/admin/roles",
                json!({ "name": "people-admin", "caps": ["mcp:members.manage:call"] }),
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
    let none = token(&key, "user:mallory", "nube", &["bus:chan/*:pub"]);
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
        "nube",
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
    let none = token(&key, "user:mallory", "nube", &["bus:chan/*:pub"]);
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
