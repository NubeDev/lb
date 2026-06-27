//! Gateway parity + the server-deny-on-forged-call for the admin-crud surface (admin-crud +
//! admin-console scopes). Proves: (a) the destructive routes are reachable for an admin, (b) a
//! **forged** call by a non-admin is denied **server-side** (the UI cap-gate is NOT the boundary),
//! and (c) the `/login` route honors `user.disable` (disable bites login over the real transport).

mod common;

use axum::http::StatusCode;
use common::{bearer, delete_req, gateway, json_post, post_empty, token, NOW};
use ed25519_dalek::{Signer, SigningKey as PublisherSigningKey};
use lb_registry::{digest, digest_hex, Artifact, PublisherKey, TrustedKeys};
use lb_role_gateway::{router, Gateway};
use serde_json::json;
use std::sync::Arc;
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

    // A fresh (un-administered) user still logs in → 200 (auto-seed preserved).
    let resp = router(gw)
        .oneshot(json_post(
            "/login",
            json!({ "user": "newcomer", "workspace": "acme" }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
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
