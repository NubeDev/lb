//! The two mandatory walls: the capability deny, and a workspace that never sees another's outcomes.
//!
//! Part of the `scorecard` suite (see `scorecard_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::scorecard_support::*;

// --- MANDATORY: capability deny -----------------------------------------------------------------

/// A principal without `mcp:rule.scorecard:call` is refused — including one that holds every OTHER
/// case cap. The scorecard is a read, but it is still a gated read.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_scorecard_denies_a_principal_without_its_cap() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let full = principal("user:test", "nube", ALL);
    outcome(
        &node,
        &full,
        "nube",
        "deny-probe",
        "rule:probe",
        None,
        1,
        "fixed",
        2,
    )
    .await;

    let bare = principal("key:nightly-rule", "nube", &[RAISE, I_GET, GET, WORKFLOW]);
    let err = call(&node, &bare, "nube", "rule.scorecard", json!({}))
        .await
        .unwrap_err();
    assert!(
        matches!(err, ToolError::Denied),
        "rule.scorecard must be Denied without its capability, got {err:?}"
    );
}

// --- MANDATORY: workspace isolation -------------------------------------------------------------

/// A ws-B principal never sees ws-A's outcomes. The cross-workspace call is refused outright, and a
/// legitimate call in B — same rule id, same site names — reports only B's own cases.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_ws_b_principal_never_sees_ws_a_outcomes() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    seed_roster(&node, "other").await;

    let a = principal("user:test", "nube", ALL);
    outcome(
        &node,
        &a,
        "nube",
        "iso-1",
        "rule:shared",
        Some("north"),
        1,
        "fixed",
        2,
    )
    .await;
    outcome(
        &node,
        &a,
        "nube",
        "iso-2",
        "rule:shared",
        Some("north"),
        1,
        "fixed",
        2,
    )
    .await;

    let b = principal("user:test", "other", ALL);
    outcome(
        &node,
        &b,
        "other",
        "iso-3",
        "rule:shared",
        Some("north"),
        1,
        "false_positive",
        2,
    )
    .await;

    // The outer gate refuses the cross-workspace call before anything is read.
    let err = call(&node, &b, "nube", "rule.scorecard", json!({}))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::Denied), "got {err:?}");

    // And B's own scorecard carries only B's single false positive — not A's two fixes.
    let rows = scorecard(&node, &b, "other", json!({})).await;
    let r = row(&rows, "rule:shared", Some("north"));
    assert_eq!(r["raised"], json!(1));
    assert_eq!(r["false_positive"], json!(1));
    assert_eq!(r["fixed"], json!(0));
    assert_eq!(r["precision"], json!(0.0));

    // A's own scorecard is untouched by B.
    let rows_a = scorecard(&node, &a, "nube", json!({})).await;
    let ra = row(&rows_a, "rule:shared", Some("north"));
    assert_eq!(ra["raised"], json!(2));
    assert_eq!(ra["precision"], json!(1.0));
}
