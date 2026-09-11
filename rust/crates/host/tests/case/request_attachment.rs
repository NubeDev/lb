//! Attachments: a single-segment asset id that rides on the reply.
//!
//! Part of the `request` suite (see `request_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::request_support::*;

// --- Attachments ------------------------------------------------------------------------------------

/// A file the contractor uploads lands in the asset store under a ONE-SEGMENT id, and the reply
/// references it. A `.` in an asset id breaks `store:asset/{id}:write` — the upload succeeds and the
/// bytes are unreadable for ever.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_attachment_gets_a_single_segment_id_and_rides_on_the_reply() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    let case_id = seed_case(&node, &full, ws, "attach-probe").await;
    seed_party(&node, &full, ws, "northern", 24).await;
    let (id, token) = send_ask(&node, &full, ws, &case_id, "northern").await;
    let (party, _) = lb_host::case_request_authenticate(&node.store, ws, &token, T0 + 1)
        .await
        .expect("resolves");

    let receipt = lb_host::case_request_attach(
        &node.store,
        &party,
        ws,
        &id,
        // A filename full of the characters that would break a record id, to prove the id is NOT
        // derived from it.
        "site.photo.v2.jpg",
        "image/jpeg",
        b"\xff\xd8\xff-not-really-a-jpeg".to_vec(),
        T0 + 2,
    )
    .await
    .expect("attach ok");
    assert!(
        !receipt.id.contains('.') && !receipt.id.contains(':'),
        "an asset id must be one record-id segment: {}",
        receipt.id
    );
    assert_eq!(
        receipt.name, "site.photo.v2.jpg",
        "the name is data, not an id"
    );

    // The bytes really are readable back — the failure the one-segment rule prevents is silent.
    let stored = lb_assets::get_asset(&node.store, ws, &receipt.id)
        .await
        .expect("read ok")
        .expect("the asset exists");
    assert_eq!(stored.bytes.len(), receipt.size);

    call(
        &node,
        &party,
        ws,
        "case.request.reply",
        json!({
            "id": id,
            "reply": { "kind": "done", "text": "photo attached", "attachments": [receipt.id] },
            "ts": T0 + 3,
        }),
    )
    .await
    .expect("reply with an attachment ok");

    let events = events_of(&node, &full, ws, &case_id).await;
    let reply_event = events
        .iter()
        .find(|e| e["kind"].as_str() == Some("reply"))
        .expect("a reply event");
    assert_eq!(
        reply_event["data"]["attachments"][0].as_str(),
        Some(receipt.id.as_str())
    );

    // A token scoped to ANOTHER request cannot upload against this one.
    let case_b = seed_case(&node, &full, ws, "attach-other").await;
    seed_party(&node, &full, ws, "southern", 24).await;
    let (_id_b, token_b) = send_ask(&node, &full, ws, &case_b, "southern").await;
    let (other, _) = lb_host::case_request_authenticate(&node.store, ws, &token_b, T0 + 1)
        .await
        .expect("resolves");
    let denied = lb_host::case_request_attach(
        &node.store,
        &other,
        ws,
        &id,
        "sneak.jpg",
        "image/jpeg",
        b"nope".to_vec(),
        T0 + 4,
    )
    .await;
    assert!(
        denied.is_err(),
        "a token may only attach to its own request"
    );
}
