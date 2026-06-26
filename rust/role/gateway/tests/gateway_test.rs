//! The SSE/HTTP gateway, end to end — the browser path that replaces the S2 in-memory UI fake.
//!
//! Request/response routes (`post`, `history`) are driven with `tower::oneshot` (no socket — the
//! axum router is a `tower::Service`). The SSE stream is tested over a REAL bound port with a
//! second session posting, proving the whole "others' live messages appear in the browser" story.
//!
//! Mandatory categories at this surface: **capability-deny** (a session without the grant gets
//! 403 from the gateway, the host's check) and **workspace-isolation** (a session scoped to ws B
//! cannot read/post ws A's channel through the gateway). The gateway adds no authority — it
//! forwards to the same capability-checked host verbs, so these mirror the host's own deny/iso.
//!
//! Boots a Node (→ a Zenoh peer) → multi-thread flavor; unique workspace id per test.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{Node, Role as NodeRole};
use lb_inbox::Item;
use lb_role_gateway::{router, Gateway};
use tower::ServiceExt; // for `oneshot`

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:browser".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

async fn gateway(ws: &str, caps: &[&str]) -> Gateway {
    let node = Node::boot_as(NodeRole::Hub).await.expect("node boots");
    Gateway::with_principal(node, principal(ws, caps), ws)
}

fn post_req(cid: &str, item: &Item) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(format!("/channels/{cid}/messages"))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(item).unwrap()))
        .unwrap()
}

fn get_req(cid: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(format!("/channels/{cid}/messages"))
        .body(Body::empty())
        .unwrap()
}

async fn json_body<T: serde::de::DeserializeOwned>(resp: axum::response::Response) -> T {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn post_then_history_round_trips_over_http() {
    // The browser's send→read path. POST a message, then GET history and see it — the S2 exit
    // gate now over a REAL transport instead of the in-memory fake.
    let gw = gateway(
        "gw-roundtrip",
        &["bus:chan/general:pub", "bus:chan/general:sub"],
    )
    .await;
    let item = Item::new("m1", "general", "user:browser", "hello over http", 1);

    let resp = router(gw.clone())
        .oneshot(post_req("general", &item))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "post accepted");
    let stored: Item = json_body(resp).await;
    assert_eq!(stored.body, "hello over http");

    let resp = router(gw).oneshot(get_req("general")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let history: Vec<Item> = json_body(resp).await;
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].body, "hello over http");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_post_without_the_grant_is_403() {
    // MANDATORY capability-deny at the gateway surface: no `pub` grant → the host denies → 403.
    let gw = gateway("gw-deny", &["bus:chan/general:sub"]).await; // sub only, no pub
    let item = Item::new("m1", "general", "user:browser", "blocked", 1);

    let resp = router(gw)
        .oneshot(post_req("general", &item))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "ungranted post is 403"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_session_in_ws_b_cannot_read_ws_a_through_the_gateway() {
    // MANDATORY workspace-isolation at the gateway, on ONE shared node: a ws_a session seeds a
    // message; a ws_b session reading the same channel name through the same node+store sees
    // NOTHING — the host's gate-1 isolation, surfaced over HTTP. Two gateways, one node, each
    // walled to its own workspace.
    let ws_a = "gw-iso-a";
    let ws_b = "gw-iso-b";
    let node = std::sync::Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));

    // Seed ws_a with a message via a ws_a gateway over the shared node.
    let gw_a = Gateway::from_shared(
        node.clone(),
        principal(ws_a, &["bus:chan/general:pub", "bus:chan/general:sub"]),
        ws_a,
    );
    let item = Item::new("m1", "general", "user:a", "ws_a secret", 1);
    let resp = router(gw_a)
        .oneshot(post_req("general", &item))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // A ws_b gateway (same node) reading `general` sees NOTHING — ws_a's data is walled off.
    let gw_b = Gateway::from_shared(
        node.clone(),
        principal(ws_b, &["bus:chan/general:sub"]),
        ws_b,
    );
    let resp = router(gw_b).oneshot(get_req("general")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let history: Vec<Item> = json_body(resp).await;
    assert!(history.is_empty(), "GATEWAY LEAK: ws_b read ws_a's items");

    // And ws_a's own read DOES see it (the empty ws_b read is isolation, not a failed write).
    let gw_a2 = Gateway::from_shared(node, principal(ws_a, &["bus:chan/general:sub"]), ws_a);
    let resp = router(gw_a2).oneshot(get_req("general")).await.unwrap();
    let a_history: Vec<Item> = json_body(resp).await;
    assert_eq!(a_history.len(), 1, "ws_a really stored its message");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_sse_stream_pushes_a_live_message_to_the_browser() {
    // The heart of S3's UI story: a browser opens the SSE stream, ANOTHER session posts, and the
    // message arrives over SSE in real time — `useChannel`'s `setItems` sink fed by the server.
    // Tested over a REAL bound port (SSE needs a live socket), with a timeout so it can't hang.
    use std::time::Duration;

    let ws = "gw-sse-live";
    let node = std::sync::Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let gw = Gateway::from_shared(
        node.clone(),
        principal(ws, &["bus:chan/general:pub", "bus:chan/general:sub"]),
        ws,
    );

    // Serve on an ephemeral port.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = router(gw);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // The browser opens the SSE stream.
    let client = reqwest::Client::new();
    let mut resp = client
        .get(format!("http://{addr}/channels/general/stream"))
        .send()
        .await
        .expect("sse stream opens");
    assert_eq!(resp.status(), 200);

    // Another session posts (directly through the host on the same shared node — simulating a
    // different client). The SSE stream must surface it.
    let poster = principal(ws, &["bus:chan/general:pub"]);
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

    // Read SSE chunks until we see the message body, with an overall timeout.
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
        "SSE framed it as a message event: {body:?}"
    );
    assert!(
        body.contains("appeared live"),
        "the posted body streamed to the browser"
    );
}
