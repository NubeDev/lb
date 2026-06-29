//! The SSE/HTTP gateway feature routes, end to end (collaboration scope, slice 1): the channel
//! registry, inbox + outbox status, and the live SSE stream. Each runs over a real session (the
//! mandatory session/deny/isolation cases live in `gateway_test.rs`; shared fixtures in
//! `tests/common/`). Split from `gateway_test.rs` to stay under the FILE-LAYOUT 400-line limit.

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::*;
use lb_auth::SigningKey;
use lb_host::{Node, Role as NodeRole};
use lb_inbox::Item;
use lb_role_gateway::router;
use tower::ServiceExt; // for `oneshot`

// ----- channel registry -------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn channel_create_then_list_shows_it_and_posting_registers_a_channel() {
    let (gw, key) = gateway().await;
    let tok = token(
        &key,
        "user:ada",
        "acme",
        &["bus:chan/*:pub", "bus:chan/*:sub"],
    );

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
            json_post(
                "/inbox/appr-1/resolve",
                serde_json::json!({ "decision": "approved" }),
            ),
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
    assert_eq!(
        res.actor, "user:ada",
        "actor is the session principal, not caller-supplied"
    );
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
    assert_eq!(
        status["pending"].as_array().unwrap().len(),
        1,
        "pending reflected"
    );
    assert_eq!(status["delivered"].as_array().unwrap().len(), 0);

    lb_outbox::mark_delivered(&node.store, "acme", "e1")
        .await
        .expect("mark delivered");

    let resp = router(gateway_on(node, &key))
        .oneshot(bearer(get_req("/outbox"), &tok))
        .await
        .unwrap();
    let status: serde_json::Value = json_body(resp).await;
    assert_eq!(
        status["pending"].as_array().unwrap().len(),
        0,
        "no longer pending"
    );
    assert_eq!(
        status["delivered"].as_array().unwrap().len(),
        1,
        "now delivered"
    );
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
    let tok = token(
        &key,
        "user:ada",
        ws,
        &["bus:chan/general:pub", "bus:chan/general:sub"],
    );

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
    let poster = lb_auth::verify(
        &key,
        &token(&key, "user:other", ws, &["bus:chan/general:pub"]),
        NOW,
    )
    .expect("poster verifies");
    lb_host::post(
        node.as_ref(),
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

    assert!(
        body.contains("event: message"),
        "framed as a message event: {body:?}"
    );
    assert!(
        body.contains("appeared live"),
        "the posted body streamed to the browser"
    );
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

// ----- tools.catalog over HTTP (channels-command-palette scope) ---------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn mcp_catalog_returns_ws_and_tools_for_a_holder_and_403s_without_the_cap() {
    let (gw, key) = gateway().await;

    // A token holding the verb gate gets 200 + `{ ws, tools }`.
    let tok = token(&key, "user:ada", "acme", &["mcp:tools.catalog:call"]);
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/mcp/catalog"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cat: serde_json::Value = json_body(resp).await;
    assert_eq!(
        cat["ws"], "acme",
        "the catalog reports the token's workspace"
    );
    assert!(cat["tools"].is_array(), "the catalog has a tools array");

    // A token WITHOUT the gate is 403-opaque.
    let no_cap = token(&key, "user:eve", "acme", &["mcp:inbox.list:call"]);
    let resp = router(gw)
        .oneshot(bearer(get_req("/mcp/catalog"), &no_cap))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "no mcp:tools.catalog:call → 403"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn mcp_catalog_is_capability_filtered_over_http() {
    let (gw, key) = gateway().await;

    // With the federation grant, `federation.query` appears in the catalog.
    let with = token(
        &key,
        "user:ada",
        "acme",
        &["mcp:tools.catalog:call", "mcp:federation.query:call"],
    );
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/mcp/catalog"), &with))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cat: serde_json::Value = json_body(resp).await;
    let names: Vec<&str> = cat["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"federation.query"),
        "the grant holder sees federation.query: {names:?}"
    );

    // Without it, the same tool is ABSENT (capability-filtered, no existence leak).
    let without = token(&key, "user:ada", "acme", &["mcp:tools.catalog:call"]);
    let resp = router(gw)
        .oneshot(bearer(get_req("/mcp/catalog"), &without))
        .await
        .unwrap();
    let cat: serde_json::Value = json_body(resp).await;
    let names: Vec<&str> = cat["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(
        !names.contains(&"federation.query"),
        "without the grant the tool is absent: {names:?}"
    );
}

// ----- post → query_error round-trip over HTTP (channels-query-charts scope) --------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn posting_a_query_item_without_the_grant_round_trips_an_opaque_query_error() {
    // The post→worker→history round-trip over the real gateway, needing NO datasource: a poster who
    // can pub/sub the channel but lacks `mcp:federation.query:call` posts a `kind:"query"` Item; the
    // inline worker denies host-side (before any sidecar) and posts a `query_error` whose message is
    // the opaque "query not permitted". History (GET) then shows BOTH items.
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let cid = "analytics";
    let tok = token(
        &key,
        "user:ada",
        "acme",
        &[
            &format!("bus:chan/{cid}:pub"),
            &format!("bus:chan/{cid}:sub"),
        ],
    );

    let body = serde_json::json!({
        "kind": "query", "source": "pg", "sql": "SELECT 1"
    })
    .to_string();
    let item = Item::new("q1", cid, "user:ada", body, 1);
    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(post_req(cid, &item), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "the query item posts");

    let resp = router(gateway_on(node, &key))
        .oneshot(bearer(get_req(&format!("/channels/{cid}/messages")), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let items: Vec<Item> = json_body(resp).await;
    let err = items
        .iter()
        .find(|i| i.author == "system:query-worker")
        .expect("history shows the worker's query_error answer");
    let payload: serde_json::Value = serde_json::from_str(&err.body).unwrap();
    assert_eq!(payload["kind"], "query_error");
    assert_eq!(
        payload["error"], "query not permitted",
        "opaque deny over HTTP: {payload}"
    );
}
