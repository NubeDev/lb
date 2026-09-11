//! The Mine lane: the caller's teams, the deadline-then-severity sort, and the party filter that refuses rather than matching everything.
//!
//! Part of the `plane` suite (see `plane_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::plane_support::*;

// --- The Mine lane ------------------------------------------------------------------------------

/// The `Mine` lane is "the person plus every team they belong to" — the SAME definition
/// `SubFilter.assignee: "me"` already ships. A naive `assigned_to == principal.sub()` silently drops
/// every case assigned to a QUEUE the caller is on, which hides exactly the team-owned work the
/// `team:` subject decision exists to support.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_mine_lane_includes_the_teams_the_caller_is_on() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    let own = seed_insight(&node, &p, "nube", "mine-own", 1).await;
    let team = seed_insight(&node, &p, "nube", "mine-team", 2).await;
    let other = seed_insight(&node, &p, "nube", "mine-other", 3).await;
    let (own_case, team_case, other_case) = (
        case_of(&node, &p, "nube", &own).await,
        case_of(&node, &p, "nube", &team).await,
        case_of(&node, &p, "nube", &other).await,
    );

    call(
        &node,
        &p,
        "nube",
        "case.assign",
        json!({ "id": own_case, "assignee": "user:priya", "ts": 4 }),
    )
    .await
    .expect("assign ok");
    call(
        &node,
        &p,
        "nube",
        "case.assign",
        json!({ "id": team_case, "assignee": "team:mechanical", "ts": 5 }),
    )
    .await
    .expect("assign ok");
    call(
        &node,
        &p,
        "nube",
        "case.assign",
        json!({ "id": other_case, "assignee": "user:test", "ts": 6 }),
    )
    .await
    .expect("assign ok");

    // Priya is on `team:mechanical`, so her lane holds BOTH her own case and the crew's.
    let priya = principal("user:priya", "nube", ALL);
    let page = call(
        &node,
        &priya,
        "nube",
        "case.list",
        json!({ "lane": "mine" }),
    )
    .await
    .expect("list ok");
    let ids: Vec<&str> = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert!(
        ids.contains(&own_case.as_str()),
        "her own case is missing: {ids:?}"
    );
    assert!(
        ids.contains(&team_case.as_str()),
        "the TEAM's case is missing — 'mine' must resolve through teams: {ids:?}"
    );
    assert!(
        !ids.contains(&other_case.as_str()),
        "somebody else's case leaked into her lane: {ids:?}"
    );

    // And the roster carries the member COUNT, never the members.
    for row in page["items"].as_array().unwrap() {
        assert!(row["member_count"].is_u64(), "no member_count on {row}");
        assert!(
            row.get("items").is_none() && row.get("members").is_none(),
            "case.list must never carry the members themselves: {row}"
        );
    }
}

/// The queue's order is the product decision: **`due_at` ascending, then severity descending**, with
/// un-clocked cases last. A `warning` due this afternoon outranks a `critical` due next month,
/// because the first is about to become a broken promise.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_lane_sorts_by_deadline_then_severity() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    // Three cases, deadlines written directly (the sla-clock reactor owns them in the product; the
    // ORDER is this verb's contract and is tested independently of who stamps the field).
    let mut ids = Vec::new();
    for (key, severity, due) in [
        ("sort-late-crit", "critical", Some(9_000u64)),
        ("sort-soon-warn", "warning", Some(1_000u64)),
        ("sort-never-crit", "critical", None),
    ] {
        let mut input = raise_input(key, 1);
        input["severity"] = json!(severity);
        let out = call(&node, &p, "nube", "insight.raise", input)
            .await
            .expect("raise ok");
        let case_id = case_of(&node, &p, "nube", out["id"].as_str().unwrap()).await;
        if let Some(due) = due {
            let mut case = lb_cases::get(&node.store, "nube", &case_id)
                .await
                .unwrap()
                .unwrap();
            case.due_at = Some(due);
            let value = serde_json::to_value(&case).unwrap();
            lb_store::write(&node.store, "nube", lb_cases::CASE_TABLE, &case_id, &value)
                .await
                .unwrap();
        }
        ids.push(case_id);
    }

    let page = call(
        &node,
        &p,
        "nube",
        "case.list",
        json!({ "lane": "watching" }),
    )
    .await
    .expect("list ok");
    let order: Vec<&str> = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        order,
        vec![ids[1].as_str(), ids[0].as_str(), ids[2].as_str()],
        "expected soon-warning, then late-critical, then the un-clocked critical: {page}"
    );
}

/// The `party` filter is REFUSED rather than silently matching everything — the `case_request` plane
/// it needs is wave 2. A filter that quietly matches every row is how a queue lies.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_party_filter_refuses_rather_than_matching_everything() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let err = call(
        &node,
        &p,
        "nube",
        "case.list",
        json!({ "lane": "watching", "filter": { "party": "party:acme-mech" } }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::BadInput(_)), "{err:?}");
}
