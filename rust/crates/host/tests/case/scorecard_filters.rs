//! The filters, and the rule that an unresolved case is never counted.
//!
//! Part of the `scorecard` suite (see `scorecard_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::scorecard_support::*;

// --- Filters -------------------------------------------------------------------------------------

/// `rule_ref`, `site` and the `since`/`until` window each narrow the read, and an OPEN case is
/// never counted — a scorecard is a fold over finished work.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_filters_narrow_and_an_unresolved_case_is_never_counted() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    outcome(
        &node,
        &p,
        "nube",
        "old",
        "rule:zeta",
        Some("north"),
        1_000,
        "fixed",
        2_000,
    )
    .await;
    outcome(
        &node,
        &p,
        "nube",
        "new",
        "rule:zeta",
        Some("north"),
        1_000,
        "false_positive",
        9_000,
    )
    .await;
    outcome(
        &node,
        &p,
        "nube",
        "other-site",
        "rule:zeta",
        Some("south"),
        1_000,
        "fixed",
        2_000,
    )
    .await;
    outcome(
        &node,
        &p,
        "nube",
        "other-rule",
        "rule:eta",
        Some("north"),
        1_000,
        "fixed",
        2_000,
    )
    .await;
    // Raised and never resolved — it has no outcome, so it contributes nothing.
    seed_insight(
        &node,
        &p,
        "nube",
        "still-open",
        "rule:zeta",
        Some("north"),
        1_000,
    )
    .await;

    let all = scorecard(&node, &p, "nube", json!({})).await;
    assert_eq!(row(&all, "rule:zeta", Some("north"))["raised"], json!(2));

    let by_rule = scorecard(&node, &p, "nube", json!({ "rule_ref": "rule:zeta" })).await;
    assert!(
        by_rule.iter().all(|r| r["rule_ref"] == json!("rule:zeta")),
        "the rule_ref filter leaked: {by_rule:#?}"
    );

    let by_site = scorecard(&node, &p, "nube", json!({ "site": "south" })).await;
    assert_eq!(by_site.len(), 1);
    assert_eq!(by_site[0]["site"], json!("south"));

    // The window is on `resolved_ts` and inclusive at both ends: only the case closed at 9_000.
    let windowed = scorecard(
        &node,
        &p,
        "nube",
        json!({ "since": 3_000, "until": 10_000 }),
    )
    .await;
    assert_eq!(windowed.len(), 1, "{windowed:#?}");
    assert_eq!(windowed[0]["rule_ref"], json!("rule:zeta"));
    assert_eq!(windowed[0]["false_positive"], json!(1));
    assert_eq!(windowed[0]["fixed"], json!(0));
}
