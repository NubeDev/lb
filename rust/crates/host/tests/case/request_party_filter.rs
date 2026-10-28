//! The `party` filter axis — "Waiting On: Northern Mechanical" (Insights review 2026-10): a case
//! matches while the party holds a live (sent or opened) request on it, and stops matching once the
//! request is withdrawn.
//!
//! Part of the `request` suite (see `request_support.rs` for the fixtures and the full preamble).
//! One binary: `case_suite.rs`.

use super::request_support::*;

async fn waiting_on(node: &Arc<Node>, p: &Principal, ws: &str, party: &str) -> Vec<String> {
    let page = call(
        node,
        p,
        ws,
        "case.list",
        json!({ "lane": "watching", "filter": { "party": party }, "now": T0 + 1 }),
    )
    .await
    .expect("a party filter is answered, not refused");
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_case_waits_on_the_party_its_live_request_went_to() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    // The request suite's caps plus the queue read the filter rides.
    let caps: Vec<&str> = ALL.iter().copied().chain(["mcp:case.list:call"]).collect();
    let full = principal("user:test", ws, &caps);
    let asked = seed_case(&node, &full, ws, "party-asked").await;
    let other = seed_case(&node, &full, ws, "party-other").await;
    seed_party(&node, &full, ws, "northern", 10).await;
    seed_party(&node, &full, ws, "southern", 10).await;

    let send = |case_id: String, party: &'static str| {
        let (node, full) = (node.clone(), full.clone());
        async move {
            call(
                &node,
                &full,
                ws,
                "case.request.send",
                json!({ "case_id": case_id, "party_id": party, "ask": "quote",
                        "brief": { "ask_text": "Please price it." }, "ts": T0 }),
            )
            .await
            .expect("send ok")
        }
    };
    let request = send(asked.clone(), "northern").await;
    send(other.clone(), "southern").await;

    assert_eq!(
        waiting_on(&node, &full, ws, "northern").await,
        vec![asked.clone()]
    );
    assert_eq!(waiting_on(&node, &full, ws, "southern").await, vec![other]);
    assert!(waiting_on(&node, &full, ws, "nobody").await.is_empty());

    call(
        &node,
        &full,
        ws,
        "case.request.withdraw",
        json!({ "id": request["id"], "ts": T0 + 1 }),
    )
    .await
    .expect("withdraw ok");
    assert!(
        waiting_on(&node, &full, ws, "northern").await.is_empty(),
        "a withdrawn request is not waited on"
    );
}
