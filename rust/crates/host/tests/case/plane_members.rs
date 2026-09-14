//! `case.members` — the citation list, and the title echo the drawer reads it by.
//!
//! Part of the `plane` suite (see `plane_support.rs` for the fixtures and the full preamble). One
//! binary: `case_suite.rs`.
//!
//! The echo exists because a membership row holds an ID. Without it the operator's drawer is a
//! column of ULIDs while an unauthenticated contractor's token page shows a headline — and the only
//! alternatives were an N+1 from the client (200 `insight.get` calls a page, over the wire, needing
//! `insight.get` caps a `case.get` holder may not have) or a stored copy that goes stale the moment
//! an insight is retitled.

use super::plane_support::*;

/// Every cited insight's headline and severity arrive WITH the page.
///
/// Asserted on a merged case so the list is more than one row, and read through the real dispatcher
/// so what is asserted is the wire shape a client actually receives.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn members_echo_each_cited_insight_title_and_severity() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let a = seed_insight(&node, &p, "nube", "echo-a", 1).await;
    let b = seed_insight(&node, &p, "nube", "echo-b", 2).await;
    let case_a = case_of(&node, &p, "nube", &a).await;
    let case_b = case_of(&node, &p, "nube", &b).await;
    call(
        &node,
        &p,
        "nube",
        "case.merge",
        json!({ "from": case_b, "into": case_a, "ts": 3 }),
    )
    .await
    .expect("merge ok");

    let page = call(
        &node,
        &p,
        "nube",
        "case.members",
        json!({ "case_id": case_a }),
    )
    .await
    .expect("members ok");

    let items = page["items"].as_array().expect("items array");
    assert_eq!(items.len(), 2, "both citations are on the page: {page}");
    for item in items {
        let id = item["insight_id"].as_str().expect("an id");
        // The membership fields are still top-level — the echo is ADDITIVE, and a client written
        // against the shipped shape must not have to change.
        assert!(
            item["role"].is_string(),
            "role survives the flatten: {item}"
        );
        assert_eq!(
            item["title"].as_str(),
            Some(format!("finding {}", if id == a { "echo-a" } else { "echo-b" }).as_str()),
            "each member carries its insight's headline: {item}"
        );
        assert_eq!(item["severity"].as_str(), Some("warning"), "{item}");
    }
}

/// The echo costs the page NOTHING in reads as it grows: one query answers every row.
///
/// Asserted by BEHAVIOUR rather than by counting queries — a case with many members still answers,
/// and every row is titled. A per-member `get` would pass this too; what it guards is the shape
/// staying correct as the page fills, which is the regression a naive rewrite would introduce.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn every_row_of_a_large_page_is_titled() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let first = seed_insight(&node, &p, "nube", "bulk-0", 1).await;
    let into = case_of(&node, &p, "nube", &first).await;
    for i in 1..12u64 {
        let id = seed_insight(&node, &p, "nube", &format!("bulk-{i}"), i + 1).await;
        let from = case_of(&node, &p, "nube", &id).await;
        call(
            &node,
            &p,
            "nube",
            "case.merge",
            json!({ "from": from, "into": into, "ts": 100 + i }),
        )
        .await
        .expect("merge ok");
    }

    let page = call(
        &node,
        &p,
        "nube",
        "case.members",
        json!({ "case_id": into }),
    )
    .await
    .expect("members ok");
    let items = page["items"].as_array().expect("items array");
    assert_eq!(items.len(), 12, "{page}");
    for item in items {
        assert!(
            item["title"]
                .as_str()
                .is_some_and(|t| t.starts_with("finding bulk-")),
            "every row of the page is titled: {item}"
        );
    }
}

/// Paging still works, and the cursor is still the member's `insight_id` — the echo must not have
/// become the thing the cursor is taken from.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_page_cursor_is_still_the_insight_id() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let a = seed_insight(&node, &p, "nube", "page-a", 1).await;
    let b = seed_insight(&node, &p, "nube", "page-b", 2).await;
    let into = case_of(&node, &p, "nube", &a).await;
    let from = case_of(&node, &p, "nube", &b).await;
    call(
        &node,
        &p,
        "nube",
        "case.merge",
        json!({ "from": from, "into": into, "ts": 3 }),
    )
    .await
    .expect("merge ok");

    let first = call(
        &node,
        &p,
        "nube",
        "case.members",
        json!({ "case_id": into, "limit": 1 }),
    )
    .await
    .expect("members ok");
    assert_eq!(
        first["total"], 2,
        "total is the whole set, not the page: {first}"
    );
    let cursor = first["next"].as_str().expect("a cursor");
    assert_eq!(
        cursor,
        first["items"][0]["insight_id"].as_str().unwrap(),
        "the cursor is the last id on the page: {first}"
    );

    let second = call(
        &node,
        &p,
        "nube",
        "case.members",
        json!({ "case_id": into, "limit": 1, "after": cursor }),
    )
    .await
    .expect("members ok");
    let next_id = second["items"][0]["insight_id"].as_str().expect("an id");
    assert_ne!(
        next_id, cursor,
        "the second page is a different member: {second}"
    );
    assert!(
        second["items"][0]["title"].is_string(),
        "and it is titled too: {second}"
    );
}

/// A member whose insight is GONE keeps its row, with no title — degrade, never blank and never
/// fail. The citation is the answer; the headline is a decoration on it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_member_whose_insight_is_gone_keeps_its_row_without_a_title() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let a = seed_insight(&node, &p, "nube", "gone-a", 1).await;
    let b = seed_insight(&node, &p, "nube", "gone-b", 2).await;
    let into = case_of(&node, &p, "nube", &a).await;
    let from = case_of(&node, &p, "nube", &b).await;
    call(
        &node,
        &p,
        "nube",
        "case.merge",
        json!({ "from": from, "into": into, "ts": 3 }),
    )
    .await
    .expect("merge ok");

    // Deleting the detection needs a cap the queue's own bundle does not carry — the two planes
    // have two gates (Resolved decision #6). A second principal, not a widened `ALL`: widening the
    // shared fixture would quietly hand every other test in this suite a capability it does not use.
    let deleter = principal("user:test", "nube", &[I_DELETE]);
    call(
        &node,
        &deleter,
        "nube",
        "insight.delete",
        json!({ "id": b }),
    )
    .await
    .expect("delete ok");

    let page = call(
        &node,
        &p,
        "nube",
        "case.members",
        json!({ "case_id": into }),
    )
    .await
    .expect("members ok");
    let items = page["items"].as_array().expect("items array");
    let orphan = items
        .iter()
        .find(|i| i["insight_id"] == json!(b))
        .unwrap_or_else(|| panic!("the citation survives the deletion: {page}"));
    assert!(
        orphan.get("title").is_none() || orphan["title"].is_null(),
        "a gone insight has no title to echo, and the field is omitted: {orphan}"
    );
    // The other row is unaffected — one missing insight must not cost the page its labels.
    let kept = items
        .iter()
        .find(|i| i["insight_id"] == json!(a))
        .expect("a is there");
    assert_eq!(kept["title"], json!("finding gone-a"), "{kept}");
}
