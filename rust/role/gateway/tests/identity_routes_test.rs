//! Gateway parity + server-deny for the global-identity surface (identity.* + membership.*). Proves:
//! (a) the routes are reachable for an admin, (b) a forged call by a non-admin is denied server-side,
//! (c) login resolves membership (bootstrap-on-empty + refuse-a-non-member), and (d) the People-tab
//! roster reads `GET /admin/members`. Real in-process gateway + store, seeded via the real routes.

mod common;

use axum::http::StatusCode;
use common::{bearer, delete_req, gateway, get_req, json_body, json_post, token, NOW};
use lb_role_gateway::router;
use serde_json::json;
use tower::ServiceExt;

const MANAGE: &[&str] = &["mcp:identity.manage:call", "mcp:members.manage:call"];

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn forged_identity_membership_call_by_non_admin_is_denied() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:mallory", "acme", &["bus:chan/*:pub"]);
    for req in [
        json_post("/admin/identities", json!({ "sub": "user:x" })),
        get_req("/admin/identities"),
        get_req("/admin/identities/user:x"),
        get_req("/admin/identities/user:x/workspaces"),
        json_post("/admin/members", json!({ "sub": "user:x" })),
        delete_req("/admin/members/user:x"),
        get_req("/admin/members"),
    ] {
        let resp = router(gw.clone()).oneshot(bearer(req, &tok)).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "forged call must be 403"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn admin_creates_identity_adds_member_lists_roster() {
    let (gw, key) = gateway().await;
    // Login registers `acme` in the node directory + bootstraps alice (the realistic first-contact
    // path), so `identity.workspaces` can scan it.
    let resp = router(gw.clone())
        .oneshot(json_post(
            "/login",
            json!({ "user": "user:alice", "workspace": "acme" }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let tok = token(&key, "user:alice", "acme", MANAGE);

    // Provision a global identity (in no workspace).
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/admin/identities",
                json!({ "sub": "user:bob", "display_name": "Bob" }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Add bob to this workspace.
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post("/admin/members", json!({ "sub": "user:bob" })),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // The roster includes bob.
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/admin/members"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let members: Vec<serde_json::Value> = json_body(resp).await;
    assert!(members.iter().any(|m| m["sub"] == "user:bob"));

    // identity.workspaces(bob) resolves acme.
    let resp = router(gw.clone())
        .oneshot(bearer(
            get_req("/admin/identities/user:bob/workspaces"),
            &tok,
        ))
        .await
        .unwrap();
    let wss: Vec<serde_json::Value> = json_body(resp).await;
    assert!(wss.iter().any(|w| w["ws"] == "acme"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn login_bootstraps_empty_workspace_and_refuses_a_non_member() {
    let (gw, _key) = gateway().await;
    // First login to a brand-new workspace bootstraps the requester (decision #3).
    let resp = router(gw.clone())
        .oneshot(json_post(
            "/login",
            json!({ "user": "user:alice", "workspace": "fresh" }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Alice is now a member of `fresh`; a different identity logging into `fresh` is refused — fresh
    // is no longer empty (decision #4).
    let resp = router(gw.clone())
        .oneshot(json_post(
            "/login",
            json!({ "user": "user:eve", "workspace": "fresh" }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn membership_remove_is_a_clean_exit() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:alice", "acme", MANAGE);
    // Add bob.
    let _ = router(gw.clone())
        .oneshot(bearer(
            json_post("/admin/members", json!({ "sub": "user:bob" })),
            &tok,
        ))
        .await
        .unwrap();
    // Remove bob — returns the revoked grant count (role:member at least).
    let resp = router(gw.clone())
        .oneshot(bearer(delete_req("/admin/members/user:bob"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let count: usize = json_body(resp).await;
    assert!(count >= 1);

    // bob's live token (minted with member_caps) is now refused on the next verify — the
    // token_revoke marker the verify chokepoint reads was written by membership.remove.
    let bob_tok = token(&key, "user:bob", "acme", MANAGE);
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/admin/members"), &bob_tok))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "bob's live token must be refused after remove"
    );
}

// Touch NOW so the const stays referenced even if the suite grows (the token helper uses it).
#[test]
fn now_const_anchor() {
    assert!(NOW > 0);
}
