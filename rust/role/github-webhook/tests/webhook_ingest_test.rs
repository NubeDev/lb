//! The GitHub webhook receiver's INGEST-BEHAVIOR tests — the happy path, idempotent re-delivery,
//! a malformed payload, and the round-trip over a REAL socket. The security-boundary categories
//! (bad-signature / deny / isolation) live in `webhook_test.rs`; the shared harness is in
//! `common/mod.rs` (split to keep each file under the 400-line FILE-LAYOUT limit).
//!
//! All driven through the REAL `github-bridge` wasm + the real store/bus. Each test boots a Node
//! (→ a Zenoh peer) → multi-thread flavor + a UNIQUE workspace id.

mod common;

use std::net::SocketAddr;

use axum::http::StatusCode;
use common::*;
use lb_host::TRIAGE_CHANNEL;
use lb_role_github_webhook::{router, WebhookState};

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_signed_delivery_ingests_one_triage_item() {
    // HAPPY PATH: a correctly-signed POST → HMAC verify → normalize → the canonical triage item.
    let ws = "wh-happy";
    let (node, state) = receiver(ws, &ingest_caps()).await;

    let body = issue_opened_webhook(7);
    assert_eq!(status(state, signed_req(&body)).await, StatusCode::OK);

    let items = lb_inbox::list(&node.store, ws, TRIAGE_CHANNEL)
        .await
        .unwrap();
    assert_eq!(items.len(), 1, "the signed delivery landed one item");
    assert_eq!(items[0].id, "acme/api#2451");
    assert!(items[0].body.contains("needs:triage"));
    assert!(items[0].body.contains("token refresh race"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn re_delivery_of_the_same_issue_is_idempotent() {
    // MANDATORY idempotent re-delivery: GitHub retries deliveries. The same issue delivered twice
    // (different `ts`, so different bytes + a different valid signature) upserts ONE inbox item —
    // the idempotency key is the normalized issue id, on the host's `(channel, id)` upsert.
    let ws = "wh-redeliver";
    let (node, _state) = receiver(ws, &ingest_caps()).await;
    let mk = || {
        WebhookState::from_shared(
            node.clone(),
            principal("user:hook", ws, &ingest_caps()),
            ws,
            SECRET,
        )
    };

    assert_eq!(
        status(mk(), signed_req(&issue_opened_webhook(7))).await,
        StatusCode::OK
    );
    assert_eq!(
        status(mk(), signed_req(&issue_opened_webhook(8))).await,
        StatusCode::OK
    );

    let items = lb_inbox::list(&node.store, ws, TRIAGE_CHANNEL)
        .await
        .unwrap();
    assert_eq!(
        items.len(),
        1,
        "a retried webhook produces exactly one item"
    );
    assert_eq!(items[0].id, "acme/api#2451");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_malformed_payload_is_422() {
    // The signature is valid (the secret-holder sent it) but the body isn't a webhook the bridge
    // can normalize → `422`, distinct from the `401` (forgery) and `403` (no grant) cases.
    let ws = "wh-malformed";
    let (node, state) = receiver(ws, &ingest_caps()).await;

    let body = r#"{"not":"a webhook"}"#;
    assert_eq!(
        status(state, signed_req(body)).await,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert!(lb_inbox::list(&node.store, ws, TRIAGE_CHANNEL)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_signed_delivery_round_trips_over_a_real_socket() {
    // The happy path over a REAL bound port (not `oneshot`) — proving the whole HTTP path, server
    // and all, is wired: bind → POST with the signature header → `200` → one item in the inbox.
    let ws = "wh-socket";
    let (node, state) = receiver(ws, &ingest_caps()).await;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let app = router(state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let body = issue_opened_webhook(9);
    let sig = signature_for(SECRET, body.as_bytes());
    let resp = raw_post(addr, &body, &sig).await;
    assert_eq!(resp, 200, "the signed delivery posted over a real socket");

    let items = lb_inbox::list(&node.store, ws, TRIAGE_CHANNEL)
        .await
        .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "acme/api#2451");
}

/// A minimal raw HTTP POST over a TcpStream (no HTTP-client dep needed in this crate's tests),
/// returning the response status code. Keeps the socket test honest without pulling a client in.
async fn raw_post(addr: SocketAddr, body: &str, signature: &str) -> u16 {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "POST /webhook HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\n\
         X-Hub-Signature-256: {signature}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut resp = Vec::new();
    stream.read_to_end(&mut resp).await.unwrap();
    let text = String::from_utf8_lossy(&resp);
    // Status line: `HTTP/1.1 200 OK`.
    text.split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0)
}
