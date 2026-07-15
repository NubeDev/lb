//! The retention reactor and the `max_samples` cap at the host seam (series-sample-cap scope,
//! issue #65).
//!
//! **The bug this pins: retention never ran.** `run_gc` was reachable only from tests and the
//! on-demand `series.retention.gc` verb — nothing ticked it at boot — so a correctly-configured
//! policy evicted NOTHING on a real node. The headline test below therefore asserts the property
//! that matters and the one a "assert spawn was called" test would miss: a series over its cap
//! shrinks to the bound **with nobody calling the verb**. Per
//! `docs/scope/ingest/drain-backpressure-scope.md`, a test asserting a PLAN never proves it
//! EXECUTES.
//!
//! Real store, real node boot, real ingest path — no mocks (testing §0).

use std::sync::Arc;
use std::time::Duration;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_ingest_tool, spawn_retention_reactors, Node};
use lb_ingest::{sample_count, set_policy, Policy, Qos, Sample};
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::json;

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

/// Commit `n` samples for `series` through the REAL ingest path (write → drain), so the rows under
/// test are exactly the rows a producer would have made.
async fn commit_samples(store: &Store, ws: &str, series: &str, n: u64) {
    let samples: Vec<Sample> = (1..=n)
        .map(|seq| Sample {
            series: series.into(),
            producer: "pi-7".into(),
            // A realistic wall-clock ts: the reactor stamps real `now`, and the cap's cutoff is on
            // the ts axis — epoch-zero rows would be a different (easier) test.
            ts: 1_784_070_000_000 + seq * 1_000,
            seq,
            payload: json!(seq),
            labels: Default::default(),
            qos: Qos::BestEffort,
        })
        .collect();
    lb_ingest::write(store, ws, &samples, 0)
        .await
        .expect("stage");
    lb_host::drain_workspace(store, ws).await.expect("drain");
}

/// THE HEADLINE — the driver actually runs. Boot a node, exceed a cap, and assert the series shrinks
/// to the bound with **nobody calling `series.retention.gc`**. This is the test the shipped
/// retention slice did not have, and its absence is why retention silently evicted nothing.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_retention_reactor_caps_a_series_with_nobody_calling_the_verb() {
    let node = Arc::new(Node::boot().await.unwrap());
    commit_samples(&node.store, "acme", "fleet.pi", 60).await;
    set_policy(
        &node.store,
        "acme",
        &Policy {
            prefix: "fleet.".into(),
            raw_for_ms: 0, // the TIME axis is off — this proves the COUNT cap ran on its own
            max_samples: 10,
            tiers: vec![],
        },
    )
    .await
    .unwrap();
    assert_eq!(
        sample_count(&node.store, "acme", "fleet.pi").await.unwrap(),
        60
    );

    // Boot the driver. Fast period for the test only — production is RETENTION_PERIOD (minutes).
    spawn_retention_reactors(
        node.clone(),
        vec!["acme".to_string()],
        Duration::from_millis(50),
    );

    let mut capped = false;
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if sample_count(&node.store, "acme", "fleet.pi").await.unwrap() == 10 {
            capped = true;
            break;
        }
    }
    assert!(
        capped,
        "the series must shrink to its bound with NO verb call — without the reactor, retention is \
         decorative and the disc fills"
    );
}

/// MANDATORY (rule 6): the reactor is workspace-scoped, not node-global. Mirrors
/// `the_ingest_reactor_only_drains_its_configured_workspace`.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_retention_reactor_only_gcs_its_configured_workspace() {
    let node = Arc::new(Node::boot().await.unwrap());
    // Identical series name, identical policy, in BOTH workspaces.
    for ws in ["ws-a", "ws-b"] {
        commit_samples(&node.store, ws, "fleet.pi", 60).await;
        set_policy(
            &node.store,
            ws,
            &Policy {
                prefix: "fleet.".into(),
                raw_for_ms: 0,
                max_samples: 10,
                tiers: vec![],
            },
        )
        .await
        .unwrap();
    }

    // Configured for ws-a ONLY.
    spawn_retention_reactors(
        node.clone(),
        vec!["ws-a".to_string()],
        Duration::from_millis(50),
    );

    let mut a_capped = false;
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if sample_count(&node.store, "ws-a", "fleet.pi").await.unwrap() == 10 {
            a_capped = true;
            break;
        }
    }
    assert!(a_capped, "the reactor must GC its configured workspace");
    assert_eq!(
        sample_count(&node.store, "ws-b", "fleet.pi").await.unwrap(),
        60,
        "ws-B's identically-named series must be untouched — the reactor is workspace-scoped, and \
         eviction must never cross the hard wall"
    );
}

/// MANDATORY (capability deny): `max_samples` is a field on an existing verb, so it mints no new
/// cap — but the existing admin gate must still cover it. A caller without
/// `mcp:series.retention.set:call` cannot set a cap, and the denial is opaque.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn setting_a_cap_without_the_admin_cap_is_denied() {
    let store = Store::memory().await.unwrap();
    let ws = "acme";
    // Holds an unrelated ingest cap — proves the gate is per-verb, not "any ingest cap will do".
    let p = principal("client:pi-7", ws, &["mcp:ingest.write:call"]);

    let err = call_ingest_tool(
        &store,
        &p,
        ws,
        "series.retention.set",
        &json!({"prefix": "fleet.", "raw_for_ms": 0, "max_samples": 10}),
    )
    .await
    .expect_err("must be denied without mcp:series.retention.set:call");
    assert!(
        matches!(err, ToolError::Denied),
        "a missing cap must DENY (opaque), never leak a different error: {err:?}"
    );

    // And with the cap, the same call succeeds and the field actually lands — a deny test only
    // proves something if the allow path works.
    let admin = principal(
        "user:ada",
        ws,
        &[
            "mcp:series.retention.set:call",
            "mcp:series.retention.list:call",
        ],
    );
    call_ingest_tool(
        &store,
        &admin,
        ws,
        "series.retention.set",
        &json!({"prefix": "fleet.", "raw_for_ms": 0, "max_samples": 10}),
    )
    .await
    .expect("the admin-tier cap admits the call");

    let listed = call_ingest_tool(&store, &admin, ws, "series.retention.list", &json!({}))
        .await
        .expect("list");
    assert_eq!(
        listed["policies"][0]["max_samples"], 10,
        "max_samples must round-trip through the MCP surface — a field the store drops silently is \
         the closed-struct trap this project has been bitten by before"
    );
}

/// The `series.retention.gc` verb reports what the cap evicted (`capped_raw`) — eviction is a policy
/// decision, but it must be OBSERVABLE, never an invisible drop.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_gc_verb_reports_what_the_cap_evicted() {
    let store = Store::memory().await.unwrap();
    let ws = "acme";
    commit_samples(&store, ws, "fleet.pi", 40).await;
    let admin = principal(
        "user:ada",
        ws,
        &[
            "mcp:series.retention.set:call",
            "mcp:series.retention.gc:call",
        ],
    );
    call_ingest_tool(
        &store,
        &admin,
        ws,
        "series.retention.set",
        &json!({"prefix": "fleet.", "raw_for_ms": 0, "max_samples": 10}),
    )
    .await
    .expect("set");

    let pass = call_ingest_tool(
        &store,
        &admin,
        ws,
        "series.retention.gc",
        &json!({"now_ms": 1_784_070_999_999u64}),
    )
    .await
    .expect("gc");
    assert_eq!(
        pass["capped_raw"], 30,
        "the pass reports the cap's evictions"
    );
    assert_eq!(sample_count(&store, ws, "fleet.pi").await.unwrap(), 10);
}
