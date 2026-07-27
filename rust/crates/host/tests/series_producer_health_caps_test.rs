//! The two MANDATORY categories for `series.producer.health` (series-observability scope, slice D):
//! **capability deny, in both directions** and **workspace isolation**.
//!
//! Split from `series_producer_health_test.rs` (which holds the contract) to keep each file inside
//! the FILE-LAYOUT budget, mirroring how `series_normalize_caps_test.rs` splits from
//! `series_normalize_test.rs`. The shared real-infra fixture is `support/producer_health.rs`.
//!
//! **Why both directions.** This verb calls extension tools on the caller's behalf. If it ran them
//! under any authority but the caller's, `mcp:series.producer.health:call` would silently become a
//! universal read of every extension on the node. A single-direction deny test passes just as
//! happily against a gate wired to the wrong capability, so the outer gate and the inner
//! per-extension gate are asserted separately — and the inner one is asserted with a permitted and a
//! forbidden extension in the SAME response, which is the only way to show the gate is per-extension
//! rather than all-or-nothing.

use std::sync::Arc;

use lb_host::Node;
use lb_mcp::ToolError;

#[path = "support/producer_health.rs"]
mod support;
use support::{admin, health, principal, register_reporter, row, seed};

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn without_the_verb_cap_the_whole_read_is_refused_opaquely() {
    // Direction 1: the outer gate. A caller holding every EXTENSION health cap but not the verb's
    // own still gets nothing — the fan-out is not a way in through the side door.
    let node = Arc::new(Node::boot().await.unwrap());
    register_reporter(&node, "demo-probe", r#"{"state":"connected"}"#, false);
    seed(&node, "acme", "ext:demo-probe/net-1", 1).await;

    let p = principal("user:bob", "acme", &["mcp:demo-probe.ingest.health:call"]);
    let err = health(&node, &p, "acme").await.unwrap_err();
    assert!(matches!(err, ToolError::Denied), "got {err:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn holding_the_verb_cap_grants_no_reach_into_an_extension_it_could_not_call() {
    // Direction 2: the inner gate, stated as the privilege-escalation question.
    let node = Arc::new(Node::boot().await.unwrap());
    register_reporter(&node, "allowed-ext", r#"{"state":"connected"}"#, false);
    register_reporter(&node, "forbidden-ext", r#"{"state":"connected"}"#, false);
    seed(&node, "acme", "ext:allowed-ext/net-1", 1).await;
    seed(&node, "acme", "ext:forbidden-ext/net-1", 2).await;

    let p = admin("acme", &["mcp:allowed-ext.ingest.health:call"]);
    let out = health(&node, &p, "acme").await.unwrap();

    assert_eq!(row(&out, "ext:allowed-ext/net-1")["state"], "reported");
    assert_eq!(row(&out, "ext:forbidden-ext/net-1")["state"], "denied");
    assert_eq!(
        row(&out, "ext:forbidden-ext/net-1")["missing_cap"],
        "mcp:forbidden-ext.ingest.health:call"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_producer_in_another_workspace_is_never_reported() {
    // The registry is node-wide, so the extension is reachable from BOTH workspaces — which is
    // exactly why this matters: the wall has to come from the SAMPLES, not from discovery.
    let node = Arc::new(Node::boot().await.unwrap());
    register_reporter(&node, "demo-probe", r#"{"state":"connected"}"#, false);
    seed(&node, "acme", "ext:demo-probe/acme-net", 1).await;
    seed(&node, "other", "ext:demo-probe/other-net", 1).await;

    let p = admin("acme", &["mcp:demo-probe.ingest.health:call"]);
    let out = health(&node, &p, "acme").await.unwrap();
    let producers: Vec<&str> = out["producers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["producer"].as_str().unwrap())
        .collect();

    assert_eq!(producers, vec!["ext:demo-probe/acme-net"]);
    assert!(
        !producers.iter().any(|p| p.contains("other-net")),
        "ws `other`'s producer leaked into ws `acme`: {producers:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_principal_cannot_read_producer_health_across_the_workspace_wall() {
    let node = Arc::new(Node::boot().await.unwrap());
    register_reporter(&node, "demo-probe", r#"{"state":"connected"}"#, false);
    seed(&node, "other", "ext:demo-probe/other-net", 1).await;

    // A ws-`acme` principal asking about ws `other` — the wall is checked before the cap.
    let p = admin("acme", &["mcp:demo-probe.ingest.health:call"]);
    let err = health(&node, &p, "other").await.unwrap_err();
    assert!(matches!(err, ToolError::Denied), "got {err:?}");
}
