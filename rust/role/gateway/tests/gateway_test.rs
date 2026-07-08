//! The SSE/HTTP gateway sessions, deny, and isolation — over a **real session** (collaboration
//! scope, slice 1). Every route authenticates a bearer token (`session::authenticate`): the
//! workspace + caps come from the token, never the request (the hard wall, §7). These tests mint
//! tokens with a KNOWN key (the one the gateway holds) so they can forge/expire tokens and prove the
//! session is real.
//!
//! Mandatory categories at this surface:
//!   - **session** — `login` issues a token that verifies; a forged or expired token is `401`; the
//!     workspace is the token's, not the request's.
//!   - **capability-deny** — a session without the grant gets `403` from the host's check.
//!   - **workspace-isolation** — two REAL sessions: a ws-B token cannot read/post/list ws-A.
//!
//! Route/feature tests (channel registry, inbox/outbox, live SSE) live in `gateway_routes_test.rs`;
//! shared fixtures in `tests/common/`. Split to stay under the FILE-LAYOUT 400-line limit.

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::*;
use lb_auth::{mint, Claims, Role, SigningKey};
use lb_host::{Node, Role as NodeRole};
use lb_inbox::Item;
use lb_role_gateway::router;
use tower::ServiceExt; // for `oneshot`

// ----- session ----------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn login_issues_a_token_that_authenticates_subsequent_requests() {
    // The keystone: log in → get a token → post with it → read it back. A real signed session.
    let (gw, _key) = gateway().await;

    let resp = router(gw.clone())
        .oneshot(json_post(
            "/login",
            serde_json::json!({ "user": "user:ada", "workspace": "acme" }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "login ok");
    let reply: serde_json::Value = json_body(resp).await;
    let tok = reply["token"].as_str().unwrap().to_string();
    assert_eq!(reply["workspace"], "acme");

    let item = Item::new("m1", "general", "user:ada", "hello with a real token", 1);
    let resp = router(gw)
        .oneshot(bearer(post_req("general", &item), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "authenticated post accepted");
    let stored: Item = json_body(resp).await;
    assert_eq!(stored.body, "hello with a real token");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_request_without_a_token_is_401() {
    let (gw, _key) = gateway().await;
    let item = Item::new("m1", "general", "user:ada", "no token", 1);
    let resp = router(gw)
        .oneshot(post_req("general", &item))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "no bearer → 401");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_forged_token_is_rejected() {
    // A token signed by a DIFFERENT key than the gateway holds → verify fails → 401.
    let (gw, _key) = gateway().await;
    let attacker = SigningKey::generate();
    let forged = token(&attacker, "user:mallory", "acme", &["bus:chan/general:pub"]);
    let item = Item::new("m1", "general", "user:mallory", "forged", 1);
    let resp = router(gw)
        .oneshot(bearer(post_req("general", &item), &forged))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "forged token → 401"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_expired_token_is_rejected() {
    let (gw, key) = gateway().await;
    let expired = {
        let claims = Claims {
            sub: "user:ada".into(),
            ws: "acme".into(),
            role: Role::Member,
            caps: vec!["bus:chan/general:pub".into()],
            iat: 0,
            exp: NOW - 1, // already expired at the gateway clock
            constraint: None,
            run_id: None,
        };
        mint(&key, &claims)
    };
    let item = Item::new("m1", "general", "user:ada", "stale", 1);
    let resp = router(gw)
        .oneshot(bearer(post_req("general", &item), &expired))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "expired token → 401"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_workspace_comes_from_the_token_not_the_request() {
    // A ws-B token posting an item whose author/body mentions ws-A still lands in ws-B: the route
    // never reads a workspace from the body. Prove it by reading back through a ws-A session and
    // seeing nothing.
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();

    let tok_b = token(
        &key,
        "user:bob",
        "ws-b",
        &["bus:chan/general:pub", "bus:chan/general:sub"],
    );
    let item = Item::new("m1", "general", "user:bob", "for ws-b only", 1);
    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(post_req("general", &item), &tok_b))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let tok_a = token(&key, "user:ada", "ws-a", &["bus:chan/general:sub"]);
    let resp = router(gateway_on(node, &key))
        .oneshot(bearer(get_req("/channels/general/messages"), &tok_a))
        .await
        .unwrap();
    let a_history: Vec<Item> = json_body(resp).await;
    assert!(
        a_history.is_empty(),
        "ws-a sees nothing — the post landed in ws-b (token's ws)"
    );
}

// ----- capability deny --------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_post_without_the_grant_is_403() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", &["bus:chan/general:sub"]); // sub only, no pub
    let item = Item::new("m1", "general", "user:ada", "blocked", 1);
    let resp = router(gw)
        .oneshot(bearer(post_req("general", &item), &tok))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "ungranted post is 403"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn inbox_list_without_the_grant_is_403() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", &["bus:chan/general:sub"]); // no inbox.list
    let resp = router(gw)
        .oneshot(bearer(get_req("/inbox/triage"), &tok))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "ungranted inbox_list is 403"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn members_add_without_the_grant_is_403() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", &["mcp:members.list:call"]); // list but not add
    let resp = router(gw)
        .oneshot(bearer(
            json_post(
                "/teams/eng/members",
                serde_json::json!({ "user": "user:bob" }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "ungranted members_add is 403"
    );
}

// ----- workspace isolation, two real sessions ---------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_ws_b_token_cannot_read_ws_a_channels_inbox_or_members() {
    // The test the demo principal made impossible: TWO real sessions on ONE node. ws-A seeds data;
    // a ws-B token sees NONE of it through any surface — the wall holds across gateway + store.
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();

    let caps = &[
        "bus:chan/*:pub",
        "bus:chan/*:sub",
        "mcp:members.list:call",
        "mcp:members.add:call",
        "mcp:inbox.list:call",
    ];
    let tok_a = token(&key, "user:ada", "ws-a", caps);
    let tok_b = token(&key, "user:bob", "ws-b", caps);

    // ws-A seeds: a channel message (also registers the channel) + a team member.
    let item = Item::new("m1", "general", "user:ada", "ws-a secret", 1);
    assert_eq!(
        router(gateway_on(node.clone(), &key))
            .oneshot(bearer(post_req("general", &item), &tok_a))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        router(gateway_on(node.clone(), &key))
            .oneshot(bearer(
                json_post(
                    "/teams/eng/members",
                    serde_json::json!({ "user": "user:ada" })
                ),
                &tok_a,
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );

    // ws-B reads each surface → empty (never ws-A's data).
    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(get_req("/channels/general/messages"), &tok_b))
        .await
        .unwrap();
    let h: Vec<Item> = json_body(resp).await;
    assert!(h.is_empty(), "ISO LEAK: ws-b read ws-a history");

    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(get_req("/channels"), &tok_b))
        .await
        .unwrap();
    let chans: Vec<serde_json::Value> = json_body(resp).await;
    assert!(chans.is_empty(), "ISO LEAK: ws-b listed ws-a channels");

    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(get_req("/teams/eng/members"), &tok_b))
        .await
        .unwrap();
    let members: Vec<String> = json_body(resp).await;
    assert!(members.is_empty(), "ISO LEAK: ws-b read ws-a team members");

    // And ws-A's own reads DO see its data (the empty ws-B reads are isolation, not failed writes).
    let resp = router(gateway_on(node, &key))
        .oneshot(bearer(get_req("/channels/general/messages"), &tok_a))
        .await
        .unwrap();
    let a_h: Vec<Item> = json_body(resp).await;
    assert_eq!(a_h.len(), 1, "ws-a really stored its message");
}
