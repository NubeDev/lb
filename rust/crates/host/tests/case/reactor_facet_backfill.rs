//! Facet backfill — the repair for cases opened before a facet key existed.
//!
//! Part of the `reactor` suite (see `reactor_support.rs` for the fixtures and the full preamble).
//! One binary: `case_suite.rs`.
//!
//! Like the two `reconcile_*` cases, `backfill_case_facets` is a node-level pass invoked as a Rust
//! function rather than a verb behind the caps wall, so there is no cap to remove and a "RED half"
//! would be theatre. Its equivalent is the **pre-pass assertion**: every case here asserts the
//! facet is genuinely absent BEFORE the pass runs, which is what makes "the pass filled it" a claim
//! the test can make rather than assume.

use super::reactor_support::*;

/// Raise through the crate with tags, then group it — the shape of a case opened while the pack was
/// still tagging only `category`/`site`/`scope`, i.e. before `subsystem` shipped.
async fn raise_tagged(
    node: &Arc<Node>,
    ws: &str,
    dedup_key: &str,
    tags: &[(&str, &str)],
    ts: usize,
) -> String {
    let mut input = legacy_raise(dedup_key, lb_insights::Severity::Warning, dedup_key);
    input.tags = tags
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    input.ts = ts as u64;
    let id = lb_insights::raise(&node.store, ws, input, ts)
        .await
        .expect("raise ok")
        .id;
    // `lb_insights::raise` deliberately does NOT write the tag echo — `input.tags` is one raise's
    // declaration, and the host layer writes the folded graph answer immediately after (see
    // `insight/raise.rs`). The fixture does the same, so the insight carries tags the way a real
    // raise leaves them.
    let folded: std::collections::BTreeMap<String, String> = tags
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    lb_insights::set_tags_echo(&node.store, ws, &id, &folded)
        .await
        .expect("tag echo ok");
    id
}

/// Read one case record straight from the crate — the durable row, not a list projection.
async fn case_row(node: &Arc<Node>, ws: &str, case_id: &str) -> lb_cases::Case {
    lb_cases::get(&node.store, ws, case_id)
        .await
        .expect("get ok")
        .expect("case exists")
}

/// The pass fills a facet the case is MISSING, from its primary insight, and is idempotent.
///
/// The fixture reproduces the real sequence: the case is opened from an insight carrying no
/// `subsystem` (so the open-time echo writes `None`), the pack then starts tagging it, and only the
/// backfill can repair the case — re-raising cannot, because the echo runs at open and only at open.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn backfill_fills_a_missing_facet_from_the_primary_insight() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;

    // Opened BEFORE `subsystem` existed: the insight carries the three facets of the day.
    let id = raise_tagged(
        &node,
        "nube",
        "pre-subsystem-1",
        &[
            ("category", "device_health"),
            ("site", "site-1"),
            ("scope", "device"),
        ],
        100,
    )
    .await;
    let case_id = lb_host::group_insight(&node, "nube", &id, 100)
        .await
        .expect("grouping ok");

    // PRE-PASS: the facet is genuinely absent, and the others are genuinely present.
    let before = case_row(&node, "nube", &case_id).await;
    assert_eq!(
        before.subsystem, None,
        "fixture must start with NO subsystem"
    );
    assert_eq!(before.category.as_deref(), Some("device_health"));
    let activity_before = before.last_activity_ts;

    // The pack ships the new key and re-raises the same finding. This alone must NOT repair the
    // case — if it did, the backfill would be unnecessary and this test would be proving nothing.
    raise_tagged(
        &node,
        "nube",
        "pre-subsystem-1",
        &[
            ("category", "device_health"),
            ("site", "site-1"),
            ("scope", "device"),
            ("subsystem", "hvac"),
        ],
        200,
    )
    .await;
    assert_eq!(
        case_row(&node, "nube", &case_id).await.subsystem,
        None,
        "a re-raise must not repair the echo; only the backfill does"
    );

    // The pass repairs it.
    let repaired = lb_host::backfill_case_facets(&node, "nube", 300)
        .await
        .expect("backfill ok");
    assert_eq!(repaired, 1, "exactly the one gapped case is repaired");
    let after = case_row(&node, "nube", &case_id).await;
    assert_eq!(after.subsystem.as_deref(), Some("hvac"));

    // A migration is not activity: the queue's activity column must not jump to the upgrade moment.
    assert_eq!(
        after.last_activity_ts, activity_before,
        "backfill must not bump last_activity_ts"
    );

    // Idempotent: nothing left to fill.
    assert_eq!(
        lb_host::backfill_case_facets(&node, "nube", 400)
            .await
            .expect("second pass ok"),
        0,
        "a second pass fills nothing"
    );
}

/// The pass fills GAPS only — it never rewrites a facet the case already carries, so a producer
/// that re-tags an insight cannot use it to move live work between queues.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn backfill_never_overwrites_a_facet_the_case_already_carries() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;

    let id = raise_tagged(
        &node,
        "nube",
        "already-tagged-1",
        &[("category", "device_health"), ("subsystem", "hvac")],
        100,
    )
    .await;
    let case_id = lb_host::group_insight(&node, "nube", &id, 100)
        .await
        .expect("grouping ok");
    assert_eq!(
        case_row(&node, "nube", &case_id).await.subsystem.as_deref(),
        Some("hvac"),
        "fixture starts with the facet already echoed"
    );

    // The insight is re-tagged to a DIFFERENT subsystem — a reclassification upstream.
    raise_tagged(
        &node,
        "nube",
        "already-tagged-1",
        &[("category", "device_health"), ("subsystem", "water")],
        200,
    )
    .await;

    assert_eq!(
        lb_host::backfill_case_facets(&node, "nube", 300)
            .await
            .expect("backfill ok"),
        0,
        "nothing is missing, so nothing is repaired"
    );
    assert_eq!(
        case_row(&node, "nube", &case_id).await.subsystem.as_deref(),
        Some("hvac"),
        "the case keeps the facet it was opened with"
    );
}

/// A CLOSED case is left alone: what a client was told at resolution is not rewritten afterwards.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn backfill_leaves_a_closed_case_alone() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    let id = raise_tagged(
        &node,
        "nube",
        "closed-1",
        &[("category", "device_health")],
        100,
    )
    .await;
    let case_id = lb_host::group_insight(&node, "nube", &id, 100)
        .await
        .expect("grouping ok");
    call(
        &node,
        &p,
        "nube",
        "case.workflow",
        json!({ "id": case_id, "workflow": "resolved", "resolution": "fixed" }),
    )
    .await
    .expect("resolve ok");

    raise_tagged(
        &node,
        "nube",
        "closed-1",
        &[("category", "device_health"), ("subsystem", "hvac")],
        200,
    )
    .await;

    assert_eq!(
        lb_host::backfill_case_facets(&node, "nube", 300)
            .await
            .expect("backfill ok"),
        0,
        "a closed case is not repaired"
    );
    assert_eq!(
        case_row(&node, "nube", &case_id).await.subsystem,
        None,
        "the resolved record is left exactly as the client saw it"
    );
}
