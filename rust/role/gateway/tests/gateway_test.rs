//! The SSE/HTTP gateway, end to end — now over a **real session** (collaboration scope, slice 1).
//!
//! Every route authenticates a bearer token (`session::authenticate`): the workspace + caps come from
//! the token, never the request (the hard wall, §7). These tests mint tokens with a KNOWN key (the
//! one the gateway holds) so they can forge/expire tokens and prove the session is real.
//!
//! Mandatory categories at this surface:
//!   - **session** — `login` issues a token that verifies; a forged or expired token is `401`; the
//!     workspace is the token's, not the request's.
//!   - **capability-deny** — a session without the grant gets `403` from the host's check.
//!   - **workspace-isolation** — two REAL sessions: a ws-B token cannot read/post/list ws-A. On one
//!     shared node, so the wall is proven across gateway + store.
//!
//! Request/response routes are driven with `tower::oneshot` (no socket); the SSE stream uses a real
//! bound port. Boots a Node (→ a Zenoh peer) → multi-thread flavor; unique workspace id per test.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use lb_auth::{mint, Claims, Role, SigningKey};
use lb_host::{Node, Role as NodeRole};
use lb_inbox::Item;
use lb_role_gateway::{router, Gateway};
use tower::ServiceExt; // for `oneshot`

/// The fixed clock the tests mint/verify at.
const NOW: u64 = 1000;

/// Build a gateway over a fresh node with a known key (so tests can mint matching tokens).
async fn gateway() -> (Gateway, SigningKey) {
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    (Gateway::new(node.clone(), key.clone(), NOW), key)
}

/// A gateway sharing a given node (two sessions, one node — the isolation setup).
fn gateway_on(node: Arc<Node>, key: &SigningKey) -> Gateway {
    Gateway::new(node, key.clone(), NOW)
}

/// Mint a token signed by `key` for `(sub, ws, caps)`, valid at `NOW`.
fn token(key: &SigningKey, sub: &str, ws: &str, caps: &[&str]) -> String {
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: NOW - 1,
        exp: NOW + 10_000,
    };
    mint(key, &claims)
}

fn bearer(req: Request<Body>, token: &str) -> Request<Body> {
    let (mut parts, body) = req.into_parts();
    parts
        .headers
        .insert("authorization", format!("Bearer {token}").parse().unwrap());
    Request::from_parts(parts, body)
}

fn post_req(cid: &str, item: &Item) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(format!("/channels/{cid}/messages"))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(item).unwrap()))
        .unwrap()
}

fn get_req(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

fn json_post(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

async fn json_body<T: serde::de::DeserializeOwned>(resp: axum::response::Response) -> T {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

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
    let resp = router(gw).oneshot(post_req("general", &item)).await.unwrap();
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
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "forged token → 401");
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
        };
        mint(&key, &claims)
    };
    let item = Item::new("m1", "general", "user:ada", "stale", 1);
    let resp = router(gw)
        .oneshot(bearer(post_req("general", &item), &expired))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "expired token → 401");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_workspace_comes_from_the_token_not_the_request() {
    // A ws-B token posting an item whose author/body mentions ws-A still lands in ws-B: the route
    // never reads a workspace from the body. Prove it by reading back through a ws-A session and
    // seeing nothing.
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();

    let tok_b = token(&key, "user:bob", "ws-b", &["bus:chan/general:pub", "bus:chan/general:sub"]);
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
    assert!(a_history.is_empty(), "ws-a sees nothing — the post landed in ws-b (token's ws)");
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
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "ungranted post is 403");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn inbox_list_without_the_grant_is_403() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", &["bus:chan/general:sub"]); // no inbox.list
    let resp = router(gw)
        .oneshot(bearer(get_req("/inbox/triage"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "ungranted inbox_list is 403");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn members_add_without_the_grant_is_403() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", &["mcp:members.list:call"]); // list but not add
    let resp = router(gw)
        .oneshot(bearer(
            json_post("/teams/eng/members", serde_json::json!({ "user": "user:bob" })),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "ungranted members_add is 403");
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
                json_post("/teams/eng/members", serde_json::json!({ "user": "user:ada" })),
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

// ----- channel registry -------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn channel_create_then_list_shows_it_and_posting_registers_a_channel() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", &["bus:chan/*:pub", "bus:chan/*:sub"]);

    // Explicit create → listed.
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post("/channels", serde_json::json!({ "channel": "hvac-alerts" })),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Post to a DIFFERENT channel → create-on-post registers it too.
    let item = Item::new("m1", "general", "user:ada", "hi", 1);
    assert_eq!(
        router(gw.clone())
            .oneshot(bearer(post_req("general", &item), &tok))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );

    let resp = router(gw)
        .oneshot(bearer(get_req("/channels"), &tok))
        .await
        .unwrap();
    let chans: Vec<serde_json::Value> = json_body(resp).await;
    let ids: Vec<&str> = chans.iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&"hvac-alerts"), "explicit create is listed");
    assert!(ids.contains(&"general"), "create-on-post is listed");
}

// ----- inbox + outbox ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn inbox_list_returns_real_items_and_resolve_persists() {
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let tok = token(
        &key,
        "user:ada",
        "acme",
        &["mcp:inbox.list:call", "mcp:inbox.resolve:call"],
    );

    // Seed a real durable inbox item directly (as the workflow would).
    lb_inbox::record(
        &node.store,
        "acme",
        &Item::new("appr-1", "approvals", "ext:github", "needs:approval", 1),
    )
    .await
    .expect("seed inbox item");

    // List shows the real item.
    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(get_req("/inbox/approvals"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let items: Vec<Item> = json_body(resp).await;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "appr-1");

    // Resolve approves it; the resolution persists.
    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(
            json_post("/inbox/appr-1/resolve", serde_json::json!({ "decision": "approved" })),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let res = lb_inbox::resolution(&node.store, "acme", "appr-1")
        .await
        .expect("read resolution")
        .expect("resolution exists");
    assert_eq!(res.decision, lb_inbox::Decision::Approved);
    assert_eq!(res.actor, "user:ada", "actor is the session principal, not caller-supplied");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn outbox_status_reflects_pending_then_delivered() {
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let tok = token(&key, "user:ada", "acme", &["mcp:outbox.status:call"]);

    // Seed a pending effect, then mark it delivered — the status view must reflect both.
    let effect = lb_outbox::Effect::new("e1", "github", "create_pr", "{}", "idem-1", 1);
    lb_outbox::enqueue(
        &node.store,
        "acme",
        "side",
        "x",
        &serde_json::json!({ "ok": true }),
        &effect,
    )
    .await
    .expect("enqueue effect");

    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(get_req("/outbox"), &tok))
        .await
        .unwrap();
    let status: serde_json::Value = json_body(resp).await;
    assert_eq!(status["pending"].as_array().unwrap().len(), 1, "pending reflected");
    assert_eq!(status["delivered"].as_array().unwrap().len(), 0);

    lb_outbox::mark_delivered(&node.store, "acme", "e1")
        .await
        .expect("mark delivered");

    let resp = router(gateway_on(node, &key))
        .oneshot(bearer(get_req("/outbox"), &tok))
        .await
        .unwrap();
    let status: serde_json::Value = json_body(resp).await;
    assert_eq!(status["pending"].as_array().unwrap().len(), 0, "no longer pending");
    assert_eq!(status["delivered"].as_array().unwrap().len(), 1, "now delivered");
}

// ----- live SSE (regression: the stream now authenticates by `?token=`) -------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_sse_stream_authenticates_by_query_token_and_pushes_a_live_message() {
    // The live-UI story over a real socket: a browser opens SSE with a `?token=` (EventSource can't
    // set a bearer header), ANOTHER session posts, and the message arrives over SSE in real time.
    use std::time::Duration;

    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let ws = "gw-sse-live";
    let tok = token(&key, "user:ada", ws, &["bus:chan/general:pub", "bus:chan/general:sub"]);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = router(gateway_on(node.clone(), &key));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // The browser opens the SSE stream with the token in the query.
    let client = reqwest::Client::new();
    let mut resp = client
        .get(format!("http://{addr}/channels/general/stream?token={tok}"))
        .send()
        .await
        .expect("sse stream opens");
    assert_eq!(resp.status(), 200);

    // Another session posts directly through the host on the shared node.
    let poster = lb_auth::verify(&key, &token(&key, "user:other", ws, &["bus:chan/general:pub"]), NOW)
        .expect("poster verifies");
    lb_host::post(
        &node.store,
        &node.bus,
        &poster,
        ws,
        "general",
        Item::new("live1", "general", "user:other", "appeared live", 1),
    )
    .await
    .expect("other session posts");

    let body = tokio::time::timeout(Duration::from_secs(5), async {
        let mut acc = String::new();
        while let Some(chunk) = resp.chunk().await.expect("read chunk") {
            acc.push_str(&String::from_utf8_lossy(&chunk));
            if acc.contains("appeared live") {
                return acc;
            }
        }
        acc
    })
    .await
    .expect("the live message arrives over SSE in time");

    assert!(body.contains("event: message"), "framed as a message event: {body:?}");
    assert!(body.contains("appeared live"), "the posted body streamed to the browser");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_sse_stream_without_a_token_is_401() {
    let (gw, _key) = gateway().await;
    let resp = router(gw)
        .oneshot(get_req("/channels/general/stream"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "no ?token= → 401");
}
