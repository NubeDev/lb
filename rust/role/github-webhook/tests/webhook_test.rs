//! The GitHub webhook receiver's SECURITY-BOUNDARY tests — the mandatory categories that guard the
//! HTTP front door. The ingest-behavior tests (happy / idempotent / malformed / real-socket) live
//! in `webhook_ingest_test.rs`; the shared harness is in `common/mod.rs` (split to keep each file
//! under the 400-line FILE-LAYOUT limit, like `github_bridge_test` + `_normalize_test`).
//!
//! Categories here, all driven through the REAL `github-bridge` wasm + the real store/bus:
//!   - **bad-signature:** a forged / tampered / absent `X-Hub-Signature-256` is `401`, never ingested;
//!   - **capability-deny:** an AUTHENTIC delivery whose principal lacks the grants is `403`;
//!   - **workspace-isolation:** a receiver fronting ws-A writes ws-A's inbox, NEVER ws-B's.
//!
//! Each test boots a Node (→ a Zenoh peer) → multi-thread flavor + a UNIQUE workspace id.

mod common;

use axum::http::StatusCode;
use common::*;
use lb_host::TRIAGE_CHANNEL;
use lb_role_github_webhook::WebhookState;

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_bad_or_missing_signature_is_401_and_ingests_nothing() {
    // MANDATORY bad-signature: a forged signature, a tampered body, and an absent header are all
    // `401` — and NONE of them reach the inbox (the cap gate is never even consulted).
    let ws = "wh-badsig";
    let (node, _state) = receiver(ws, &ingest_caps()).await;
    let mk = || {
        WebhookState::from_shared(
            node.clone(),
            principal("user:hook", ws, &ingest_caps()),
            ws,
            SECRET,
        )
    };
    let body = issue_opened_webhook(1);

    // Forged signature (signed with the wrong secret).
    let forged = signature_for(b"not-the-secret", body.as_bytes());
    assert_eq!(
        status(mk(), webhook_req(&body, Some(&forged))).await,
        StatusCode::UNAUTHORIZED
    );
    // Tampered body: a valid signature for a DIFFERENT body.
    let sig_for_other = signature_for(SECRET, issue_opened_webhook(2).as_bytes());
    assert_eq!(
        status(mk(), webhook_req(&body, Some(&sig_for_other))).await,
        StatusCode::UNAUTHORIZED
    );
    // No signature header at all.
    assert_eq!(
        status(mk(), webhook_req(&body, None)).await,
        StatusCode::UNAUTHORIZED
    );

    // Nothing was ingested by any of the rejected deliveries.
    let items = lb_inbox::list(&node.store, ws, TRIAGE_CHANNEL)
        .await
        .unwrap();
    assert!(items.is_empty(), "no rejected delivery reached the inbox");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_authentic_delivery_without_the_grants_is_403() {
    // MANDATORY capability-deny: the signature is VALID (it really is GitHub), but the receiver's
    // principal holds no grants. The host's caps gate refuses → `403`, distinct from the `401`
    // forgery case, and nothing is ingested. The signature gate is authenticity, not authority.
    let ws = "wh-deny";
    let (node, state) = receiver(ws, &[]).await; // a principal with NO grants

    let body = issue_opened_webhook(1);
    assert_eq!(
        status(state, signed_req(&body)).await,
        StatusCode::FORBIDDEN
    );

    let items = lb_inbox::list(&node.store, ws, TRIAGE_CHANNEL)
        .await
        .unwrap();
    assert!(items.is_empty(), "a denied delivery ingests nothing");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_ws_a_receiver_never_writes_ws_b() {
    // MANDATORY workspace-isolation: two receivers front the SAME node, one bound to ws-A, one to
    // ws-B. A delivery to the ws-A receiver lands in ws-A's inbox and leaves ws-B's untouched (the
    // receiver writes the fixed `ws` it was built with — the wall is the principal+ws, not the
    // shared node or the shared, node-global bridge instance).
    let ws_a = "wh-iso-a";
    let ws_b = "wh-iso-b";
    let (node, recv_a) = receiver(ws_a, &ingest_caps()).await;
    install_bridge(&node, ws_b).await.unwrap();

    let body = issue_opened_webhook(1);
    assert_eq!(status(recv_a, signed_req(&body)).await, StatusCode::OK);

    let a_items = lb_inbox::list(&node.store, ws_a, TRIAGE_CHANNEL)
        .await
        .unwrap();
    let b_items = lb_inbox::list(&node.store, ws_b, TRIAGE_CHANNEL)
        .await
        .unwrap();
    assert_eq!(a_items.len(), 1, "ws-A's receiver wrote ws-A's inbox");
    assert!(b_items.is_empty(), "ws-B's inbox is untouched");
}
