//! The authenticated `/admin/invites*` surface (invite-admin-routes scope) — the browser half of
//! the shipped invite verb family, and the **mandatory** test categories for it: a capability-deny
//! per route, the two-cap asymmetry, workspace isolation, and the mint → accept round trip. The
//! revoke/resend/hygiene/filter behaviours live in `invite_admin_lifecycle_test.rs`; shared
//! fixtures in `common/invites.rs`.
//!
//! Drives the REAL router over a REAL embedded node (`mem://` store, real caps, real principals):
//! no mocks, no fake invite service (CLAUDE §9).
//!
//! The cap matrix is the load-bearing detail: `invite.list` is gated `mcp:invite.list:call` while
//! `invite.create`/`revoke`/`resend` are gated `mcp:invite.create:call`
//! (`host/src/authz/builtin_roles.rs`). Two caps, not one — the deny tests are written per route so
//! the asymmetry stays pinned.

mod common;

use axum::http::StatusCode;
use common::invites::{hash_of, mint, roster, BOTH, CREATE, LIST};
use common::{bearer, gateway, get_req, json_body, json_post, post_empty, token};
use lb_role_gateway::router;
use serde_json::json;
use tower::ServiceExt;

// ---------------------------------------------------------------------------------------------
// Capability deny — one per route (the category that must not be skipped for an admin surface).
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn every_admin_invite_route_denies_a_principal_without_the_cap() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:mallory", "nube", &["bus:chan/*:pub"]);
    for req in [
        json_post("/admin/invites", json!({ "email": "bob@nube.com" })),
        get_req("/admin/invites"),
        post_empty("/admin/invites/deadbeef/revoke"),
        post_empty("/admin/invites/deadbeef/resend"),
    ] {
        let resp = router(gw.clone()).oneshot(bearer(req, &tok)).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "a principal holding neither invite cap must be 403 on every route"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_two_invite_caps_are_not_interchangeable() {
    // The asymmetry this test exists to pin: `list` is gated `mcp:invite.list:call`, the three
    // mutating routes `mcp:invite.create:call`. Holding one is NOT holding the other, so a caller
    // granted only the mint cap must still be denied the roster (and vice versa).
    let (gw, key) = gateway().await;

    let create_only = token(&key, "user:test", "nube", &[CREATE]);
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/admin/invites"), &create_only))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "`mcp:invite.create:call` alone must NOT open GET /admin/invites"
    );

    let list_only = token(&key, "user:lee", "nube", &[LIST]);
    for req in [
        json_post("/admin/invites", json!({ "email": "bob@nube.com" })),
        post_empty("/admin/invites/deadbeef/revoke"),
        post_empty("/admin/invites/deadbeef/resend"),
    ] {
        let resp = router(gw.clone())
            .oneshot(bearer(req, &list_only))
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "`mcp:invite.list:call` alone must NOT open the mutating routes"
        );
    }

    // ...and no invite was written by any of the denied calls.
    let admin = token(&key, "user:root", "nube", BOTH);
    assert!(
        roster(&gw, &admin, "").await.is_empty(),
        "a denied mint must leave no record behind"
    );
}

// ---------------------------------------------------------------------------------------------
// Workspace isolation — the hard wall (ws comes from the bearer, never the body or the path).
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_invite_minted_in_nube_is_invisible_and_unrevokable_from_beta() {
    let (gw, key) = gateway().await;
    let nube = token(&key, "user:test", "nube", BOTH);
    let beta = token(&key, "user:eve", "beta", BOTH);

    let _raw = mint(&gw, &nube, json!({ "email": "bob@nube.com" })).await;
    let hash = hash_of(&roster(&gw, &nube, "").await, "bob@nube.com");

    // Absent from beta's roster.
    let beta_rows = roster(&gw, &beta, "").await;
    assert!(
        beta_rows.is_empty(),
        "beta must not see nube's invites: {beta_rows:?}"
    );

    // Revoking nube's hash from beta is 404 — not 403, and not a success. A cross-workspace caller
    // must not learn whether the hash exists at all.
    let resp = router(gw.clone())
        .oneshot(bearer(
            post_empty(&format!("/admin/invites/{hash}/revoke")),
            &beta,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "cross-workspace revoke must be an opaque 404, never an existence oracle"
    );

    // And the invite is untouched in nube.
    let still = roster(&gw, &nube, "?status=pending").await;
    assert_eq!(still.len(), 1, "nube's invite must survive beta's attempt");
}

// ---------------------------------------------------------------------------------------------
// Round trip — the case that proves the two halves of the door line up.
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn mint_then_accept_onboards_the_invitee_and_flips_the_record_to_accepted() {
    let (gw, key) = gateway().await;
    let admin = token(&key, "user:test", "nube", BOTH);

    let raw = mint(
        &gw,
        &admin,
        json!({ "email": "bob@nube.com", "role": "member" }),
    )
    .await;

    // Redeem through the REAL pre-auth route.
    let resp = router(gw.clone())
        .oneshot(json_post(
            "/public/invite/accept",
            json!({ "token": raw, "workspace": "nube", "secret": "hunter2hunter2" }),
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "the minted token must redeem"
    );
    let accepted: serde_json::Value = json_body(resp).await;
    assert_eq!(accepted["principal"], "user:bob@nube.com");
    assert!(
        accepted["caps"]
            .as_array()
            .expect("caps array")
            .iter()
            .any(|c| c.as_str().is_some()),
        "the new member's caps resolve live on first login"
    );

    // The identity exists globally, the membership exists in nube, and the invite's role is granted.
    let manage = token(
        &key,
        "user:test",
        "nube",
        &["mcp:identity.manage:call", "mcp:members.manage:call"],
    );
    let resp = router(gw.clone())
        .oneshot(bearer(
            get_req("/admin/identities/user:bob@nube.com"),
            &manage,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let identity: serde_json::Value = json_body(resp).await;
    assert_eq!(identity["sub"], "user:bob@nube.com", "identity was created");

    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/admin/members"), &manage))
        .await
        .unwrap();
    let members: Vec<serde_json::Value> = json_body(resp).await;
    assert!(
        members.iter().any(|m| m["sub"] == "user:bob@nube.com"),
        "the invitee joined nube: {members:?}"
    );

    let resp = router(gw.clone())
        .oneshot(bearer(
            get_req("/admin/grants?subject=user:bob@nube.com"),
            &token(&key, "user:test", "nube", &["mcp:grants.list:call"]),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let grants: Vec<String> = json_body(resp).await;
    assert!(
        grants.iter().any(|g| g == "role:member"),
        "the invite's role was granted: {grants:?}"
    );

    // The roster now shows the record as accepted (and it is gone from `?status=pending`).
    let rows = roster(&gw, &admin, "").await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["status"], "accepted");
    assert_eq!(rows[0]["accepted_by"], "user:bob@nube.com");
    assert!(
        roster(&gw, &admin, "?status=pending").await.is_empty(),
        "an accepted invite is no longer pending"
    );
    assert_eq!(roster(&gw, &admin, "?status=accepted").await.len(), 1);
}
