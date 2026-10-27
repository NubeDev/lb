//! `case.pin` through the real dispatcher: a pin is the caller's own. A viewer with only the read
//! grant may pin; `case.list`'s `pinned` axis shows each caller their own pins and nobody else's, and
//! a pin adds nothing to the case's history.

use super::sla_clock_support::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_pin_is_the_callers_own() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "acme";
    seed_roster(&node, ws).await;
    let admin = principal("user:admin", ws, ALL);
    let case = raise_and_case(&node, &admin, ws, "k1", "warning", FRIDAY_1600).await;
    let case_id = case["id"].as_str().unwrap().to_string();

    // RED: no read grant, no pin.
    let blind = principal("user:blind", ws, &[]);
    let refused = call(
        &node,
        &blind,
        ws,
        "case.pin",
        json!({ "id": case_id, "pinned": true }),
    )
    .await;
    assert!(matches!(refused, Err(ToolError::Denied)), "got {refused:?}");

    // A viewer who may list the queue may bookmark in it.
    let viewer = principal("user:viewer", ws, &[LIST, GET]);
    let pinned = call(
        &node,
        &viewer,
        ws,
        "case.pin",
        json!({ "id": case_id, "pinned": true }),
    )
    .await
    .expect("case.pin");
    assert_eq!(pinned["pinned_by"], json!(["user:viewer"]));

    let mine = |p: &Principal| {
        let node = node.clone();
        let p = p.clone();
        async move {
            let page = call(
                &node,
                &p,
                ws,
                "case.list",
                json!({ "lane": "watching", "filter": { "pinned": true }, "limit": 50 }),
            )
            .await
            .expect("case.list");
            page["items"].as_array().unwrap().len()
        }
    };
    assert_eq!(mine(&viewer).await, 1, "the viewer sees their pin");
    assert_eq!(mine(&admin).await, 0, "the admin did not pin it");

    let history = call(
        &node,
        &admin,
        ws,
        "case.events",
        json!({ "case_id": case_id, "limit": 50 }),
    )
    .await
    .expect("case.events");
    let kinds: Vec<_> = history["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["kind"].clone())
        .collect();
    assert!(
        !kinds.contains(&json!("pinned")),
        "a pin is not in the history: {kinds:?}"
    );
}
