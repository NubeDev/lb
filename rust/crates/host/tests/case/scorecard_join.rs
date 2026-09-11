//! The join the scorecard IS: origin.ref x resolution — precision, the no-opinion denominator, the median, the grouping, and a deleted primary.
//!
//! Part of the `scorecard` suite (see `scorecard_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::scorecard_support::*;

// --- The join: origin.ref × resolution -----------------------------------------------------------

/// The scope's formula, end to end through real records: two fixes against one false positive and
/// one self-cleared case ⇒ 0.5, and `accepted_risk`/`duplicate` are counted without moving it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn precision_is_fixed_over_fixed_plus_false_positive_plus_self_cleared() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let site = Some("north");

    for (key, resolution) in [
        ("f1", "fixed"),
        ("f2", "fixed"),
        ("fp", "false_positive"),
        ("sc", "self_cleared"),
        ("ar", "accepted_risk"),
        ("dup", "duplicate"),
    ] {
        outcome(
            &node,
            &p,
            "nube",
            key,
            "rule:alpha",
            site,
            1,
            resolution,
            10,
        )
        .await;
    }

    let rows = scorecard(&node, &p, "nube", json!({})).await;
    let r = row(&rows, "rule:alpha", site);
    assert_eq!(r["raised"], json!(6));
    assert_eq!(r["fixed"], json!(2));
    assert_eq!(r["false_positive"], json!(1));
    assert_eq!(r["self_cleared"], json!(1));
    assert_eq!(r["accepted_risk"], json!(1));
    assert_eq!(r["duplicate"], json!(1));
    // 2 / (2 + 1 + 1) — the accepted risk and the duplicate are reported but out of the denominator.
    assert_eq!(r["precision"], json!(0.5));
}

/// A rule whose only resolved cases are `accepted_risk`/`duplicate` has a ZERO denominator, and the
/// verb says so by OMITTING `precision` — never `0.0`, which a UI would render as "always wrong"
/// and defame a detector that has simply never been judged.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_zero_denominator_renders_as_no_opinion_never_zero() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    outcome(
        &node,
        &p,
        "nube",
        "ar",
        "rule:beta",
        None,
        1,
        "accepted_risk",
        5,
    )
    .await;
    outcome(
        &node,
        &p,
        "nube",
        "dup",
        "rule:beta",
        None,
        1,
        "duplicate",
        5,
    )
    .await;

    let rows = scorecard(&node, &p, "nube", json!({})).await;
    let r = row(&rows, "rule:beta", None);
    assert_eq!(r["raised"], json!(2));
    assert!(
        r.get("precision").is_none(),
        "a zero denominator must omit `precision`, got {r:#?}"
    );
    assert_ne!(r["precision"], json!(0.0));
}

/// The median time-to-resolve rides the real `opened_ts`/`resolved_ts` a case carries, and only the
/// `fixed` cases vote: a 1 ms false positive beside three fixes of 100/200/300 ms must not drag the
/// "how long does a fix take" answer below 200.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_median_is_over_the_fixed_cases_only() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    for (key, opened, resolved) in [
        ("m1", 1_000, 1_100),
        ("m2", 1_000, 1_300),
        ("m3", 1_000, 1_200),
    ] {
        outcome(
            &node,
            &p,
            "nube",
            key,
            "rule:gamma",
            None,
            opened,
            "fixed",
            resolved,
        )
        .await;
    }
    outcome(
        &node,
        &p,
        "nube",
        "mfp",
        "rule:gamma",
        None,
        1_000,
        "false_positive",
        1_001,
    )
    .await;

    let rows = scorecard(&node, &p, "nube", json!({})).await;
    // 100/200/300 — the odd-count median is the middle value; the 1 ms false positive did not vote.
    assert_eq!(
        row(&rows, "rule:gamma", None)["median_resolve_ms"],
        json!(200)
    );

    // A fourth fix makes it even: (200 + 300) / 2, the MEAN of the two middle values.
    outcome(
        &node,
        &p,
        "nube",
        "m4",
        "rule:gamma",
        None,
        1_000,
        "fixed",
        1_500,
    )
    .await;
    let rows = scorecard(&node, &p, "nube", json!({})).await;
    assert_eq!(
        row(&rows, "rule:gamma", None)["median_resolve_ms"],
        json!(250)
    );
}

/// Grouping is by the PAIR, and the site-less group survives: one rule at two sites plus one case
/// with no site is three rows, and the no-site row is neither folded into a sited one nor dropped.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn grouping_is_by_rule_ref_and_site_and_the_no_site_group_survives() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    outcome(
        &node,
        &p,
        "nube",
        "n1",
        "rule:delta",
        Some("north"),
        1,
        "fixed",
        5,
    )
    .await;
    outcome(
        &node,
        &p,
        "nube",
        "s1",
        "rule:delta",
        Some("south"),
        1,
        "false_positive",
        5,
    )
    .await;
    outcome(
        &node,
        &p,
        "nube",
        "x1",
        "rule:delta",
        None,
        1,
        "self_cleared",
        5,
    )
    .await;

    let rows = scorecard(&node, &p, "nube", json!({})).await;
    assert_eq!(rows.len(), 3, "three groups, not two: {rows:#?}");
    assert_eq!(
        row(&rows, "rule:delta", Some("north"))["precision"],
        json!(1.0)
    );
    assert_eq!(
        row(&rows, "rule:delta", Some("south"))["precision"],
        json!(0.0)
    );
    let none_row = row(&rows, "rule:delta", None);
    assert_eq!(none_row["self_cleared"], json!(1));
    assert_eq!(none_row["precision"], json!(0.0));
}

/// A resolved case whose primary insight has since been DELETED is counted under `unknown`, not
/// silently dropped. The work happened; the detector's name did not survive. Dropping it would
/// shrink the denominator and make every surviving precision a lie about a smaller population than
/// the reader believes they are looking at.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_deleted_primary_insight_is_counted_under_unknown_not_dropped() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    outcome(&node, &p, "nube", "keep", "rule:eps", None, 1, "fixed", 5).await;
    let doomed = outcome(
        &node,
        &p,
        "nube",
        "gone",
        "rule:eps",
        None,
        1,
        "false_positive",
        5,
    )
    .await;

    // The record disappears; its CASE, and the outcome somebody recorded on it, do not.
    lb_insights::delete(&node.store, "nube", &doomed)
        .await
        .expect("insight deleted");

    let rows = scorecard(&node, &p, "nube", json!({})).await;
    // The named rule keeps only the outcome it can still be credited with.
    let named = row(&rows, "rule:eps", None);
    assert_eq!(named["raised"], json!(1));
    assert_eq!(named["fixed"], json!(1));
    assert_eq!(named["precision"], json!(1.0));
    // And the orphan is visible rather than vanished.
    let unknown = row(&rows, lb_host::UNKNOWN_RULE_REF, None);
    assert_eq!(unknown["raised"], json!(1));
    assert_eq!(unknown["false_positive"], json!(1));
    assert_eq!(unknown["precision"], json!(0.0));
}
