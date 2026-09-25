//! The case lens on `insight.list` over a REAL booted node, through the real MCP bridge: the case
//! stage filter and per-stage counts come from real cases the grouping reactor opened, moved by the
//! real `case.workflow` verb. No mocks (CLAUDE §9).
//!
//! What must hold: a stage filter keeps exactly the detections whose case is at that stage, the
//! per-stage tally counts every stage whatever is picked, and a caller without `case.list` is refused
//! rather than learning case stages through detection counts.

#[path = "case/plane_support.rs"]
mod plane_support;

use plane_support::*;

const WS: &str = "acme";
const I_LIST: &str = "mcp:insight.list:call";

async fn list(node: &Arc<Node>, p: &Principal, input: Value) -> Result<Value, ToolError> {
    call(node, p, WS, "insight.list", input).await
}

fn ids(page: &Value) -> Vec<String> {
    let mut v: Vec<String> = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap().to_string())
        .collect();
    v.sort();
    v
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_lens_filters_and_counts_by_the_real_case_stage() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let mut caps = ALL.to_vec();
    caps.push(I_LIST);
    let p = principal("user:test", WS, &caps);

    let a = seed_insight(&node, &p, WS, "lens:a", 10).await;
    let b = seed_insight(&node, &p, WS, "lens:b", 20).await;
    let case_a = case_of(&node, &p, WS, &a).await;
    let case_b = case_of(&node, &p, WS, &b).await;
    assert_ne!(case_a, case_b, "two unrelated findings open two cases");
    call(
        &node,
        &p,
        WS,
        "case.workflow",
        json!({ "id": case_a, "workflow": "actioned", "ts": 30 }),
    )
    .await
    .expect("move to actioned");

    // Page 1: counts asked, the "actioned" stage picked.
    let page = list(
        &node,
        &p,
        json!({ "limit": 50, "counts": true, "case": { "stages": ["actioned"] } }),
    )
    .await
    .expect("list with a lens");
    assert_eq!(ids(&page), [a.clone()]);
    // The tally ignores the picked stage: both stages are counted.
    assert_eq!(page["case_counts"]["actioned"], 1);
    assert_eq!(page["case_counts"]["to_action"], 1);
    assert_eq!(page["case_counts"]["none"], 0);

    // A later page asks for no counts and carries none.
    let page = list(
        &node,
        &p,
        json!({ "limit": 50, "case": { "stages": ["to_action", "none"] } }),
    )
    .await
    .expect("list without counts");
    assert_eq!(ids(&page), [b.clone()]);
    assert!(page.get("case_counts").is_none() && page.get("counts").is_none());

    // The queue's own filter: `workflow` narrows through the case, like `case.list`.
    let page = list(
        &node,
        &p,
        json!({ "limit": 50, "case": { "filter": { "workflow": "to_action" } } }),
    )
    .await
    .expect("list with a case filter");
    assert_eq!(ids(&page), [b]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_lens_needs_the_case_read_grant_and_a_known_stage() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let mut caps = ALL.to_vec();
    caps.push(I_LIST);
    let seeder = principal("user:test", WS, &caps);
    seed_insight(&node, &seeder, WS, "lens:c", 10).await;

    // May list detections, may not list cases: the lens is refused, a plain list is not.
    let reader = principal("user:viewer", WS, &[I_LIST]);
    let err = list(&node, &reader, json!({ "case": { "stages": ["actioned"] } }))
        .await
        .expect_err("no case.list ⇒ no lens");
    assert!(matches!(err, ToolError::Denied), "{err:?}");
    list(&node, &reader, json!({ "limit": 5 }))
        .await
        .expect("a plain list still works");

    let err = list(&node, &seeder, json!({ "case": { "stages": ["closed"] } }))
        .await
        .expect_err("an unknown stage is refused, not matched against nothing");
    assert!(matches!(err, ToolError::BadInput(_)), "{err:?}");
}
