//! The POSITIVE gate test, and the cap's place in the viewer bundle.
//!
//! Part of the `scorecard` suite (see `scorecard_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::scorecard_support::*;

// --- MANDATORY: the POSITIVE gate test ----------------------------------------------------------

/// The test a cap-in-no-bundle (or a missing `tool_gate.rs` arm) fails and nothing else does.
///
/// `rule.scorecard` gates on its OWN name, so it needs no alias — but it DOES need
/// `mcp:rule.scorecard:call` to live in the viewer bundle and the verb to be dispatched by exact
/// name in `tool_call.rs`. Miss either and every caller, admins included, gets a bare `Denied` that
/// looks exactly like a real authorization failure. Only a call that MUST succeed catches it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_principal_holding_only_the_scorecard_cap_can_actually_call_it() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let full = principal("user:test", "nube", ALL);
    outcome(
        &node,
        &full,
        "nube",
        "gate-1",
        "rule:gate",
        None,
        1,
        "fixed",
        5,
    )
    .await;

    // Nothing but the scorecard cap — no case read, no case write, no insight cap.
    let reader = principal("user:viewer", "nube", &[SCORECARD]);
    let out = call(&node, &reader, "nube", "rule.scorecard", json!({}))
        .await
        .expect("rule.scorecard must be reachable holding only its own cap");
    let rows = out["rows"].as_array().cloned().unwrap_or_default();
    assert_eq!(row(&rows, "rule:gate", None)["fixed"], json!(1));
}

/// And the cap is in the SHIPPED viewer bundle, not merely in a hand-written token — a verb only a
/// bespoke cap list can reach is shipped-but-unusable.
#[test]
fn the_scorecard_cap_ships_in_the_viewer_bundle() {
    let viewer = lb_host::viewer_role_caps();
    assert!(
        viewer.iter().any(|c| c == SCORECARD),
        "mcp:rule.scorecard:call must be in VIEWER_CAPS; viewer holds {viewer:?}"
    );
}
