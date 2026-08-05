//! **The boot wiring itself** (testing-scope §3.2, row 4; issue
//! [#108](https://github.com/NubeDev/lb/issues/108) "assert the boot wiring").
//!
//! The standing hole this file closes: deleting a `spawn_*_reactors` line from
//! `node/src/reactors.rs` broke **no test**. Each reactor's mechanism is covered in
//! `crates/host/tests/*` by spawning that one reactor directly — which is exactly the test that
//! cannot see a missing boot line, because the test supplies the line itself.
//!
//! So every test here calls the ONE real wiring entry point, [`lb_node::reactors::spawn`], and never
//! an individual spawner. Re-listing the spawners in a test would reproduce the bug this file exists
//! to kill: the list would stay green while `reactors.rs` lost an entry.
//!
//! Each test then asserts the reactor's property holds with **nobody calling its verb** — a test
//! asserting a PLAN never proves it EXECUTES (`docs/scope/ingest/drain-backpressure-scope.md`).
//!
//! Real node boot (`mem://`), real ingest write path, real reactor cadences. No mocks (testing §0).

use std::sync::Arc;
use std::time::Duration;

use lb_ingest::{sample_count, set_policy, Policy, Qos, Sample};
use lb_node::config::OutboxProviders;
use lb_node::Node;
use serde_json::json;

/// A constant sample-time base (determinism §3) at wall-clock scale: the reactors stamp a real
/// `now`, so epoch-zero rows would be a different (easier) test.
const FIRST_TS_MS: u64 = 1_784_070_000_000;

/// The boot wiring under test. **Do not inline a list of `spawn_*_reactors` calls here** — the whole
/// point is that the assertion runs through the real function, so deleting a line in `reactors.rs`
/// fails these tests. `OutboxProviders::default()` is the unconfigured embedder (the relay falls
/// back to its logging no-ops), which is all these two properties need.
async fn boot_wiring(node: &Arc<Node>, ws: &str) {
    boot_wiring_every(node, ws, None).await;
}

/// The same boot wiring with an explicit retention cadence — `None` is the shipped 300 s default.
/// Threading the parameter rather than adding a second `spawn` call keeps the rule above intact:
/// every test still boots through the one real function.
async fn boot_wiring_every(node: &Arc<Node>, ws: &str, retention_period: Option<Duration>) {
    lb_node::reactors::spawn(
        node,
        ws,
        &OutboxProviders::default(),
        None,
        None,
        None,
        retention_period,
    )
    .await;
}

/// `n` samples STAGED for `series` — written, deliberately not drained, so the only thing that can
/// commit them is a background driver.
fn staged(series: &str, n: u64) -> Vec<Sample> {
    (0..n)
        .map(|i| Sample {
            series: series.into(),
            producer: "pi-7".into(),
            ts: FIRST_TS_MS + i * 1_000,
            seq: i + 1,
            payload: json!(i),
            labels: Default::default(),
            qos: Qos::BestEffort,
        })
        .collect()
}

/// Poll `check` until it holds or ~20s elapse. Both reactors' `tokio::time::interval` fires its
/// FIRST tick immediately, so the property lands long before this — but poll-with-timeout rather
/// than sleeping a fixed guess (determinism §3).
async fn eventually<F, Fut>(mut check: F) -> bool
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    for _ in 0..200 {
        if check().await {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

/// BOOT WIRING — the ingest drain. Staged samples become committed `series` rows with **nobody
/// calling a drain**. Fails if `spawn_ingest_reactors` leaves `reactors.rs`.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn boot_spawns_the_ingest_drain_so_staged_samples_commit_with_nobody_draining() {
    let node = Arc::new(Node::boot().await.unwrap());
    let ws = "acme";
    lb_ingest::write(&node.store, ws, &staged("fleet.pi", 40), 0)
        .await
        .expect("stage");
    assert_eq!(
        sample_count(&node.store, ws, "fleet.pi").await.unwrap(),
        0,
        "nothing is committed yet — the samples are staged only"
    );

    boot_wiring(&node, ws).await;

    let committed =
        eventually(|| async { sample_count(&node.store, ws, "fleet.pi").await.unwrap() == 40 })
            .await;
    assert!(
        committed,
        "boot must spawn the ingest drain: without it staged samples commit only when some caller \
         pays for the whole backlog inside its own request, which is the bug the reactor exists to \
         fix"
    );
}

/// BOOT WIRING — the retention GC. A series over its `max_samples` cap shrinks to the bound with
/// **nobody calling `series.retention.gc`**. Fails if `spawn_retention_reactors` leaves
/// `reactors.rs`.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn boot_spawns_the_retention_gc_so_a_capped_series_shrinks_with_nobody_calling_the_verb() {
    let node = Arc::new(Node::boot().await.unwrap());
    let ws = "acme";
    lb_ingest::write(&node.store, ws, &staged("fleet.pi", 60), 0)
        .await
        .expect("stage");
    lb_host::drain_workspace(&node.store, ws)
        .await
        .expect("commit the history up front — this test is about the GC, not the drain");
    set_policy(
        &node.store,
        ws,
        &Policy {
            prefix: "fleet.".into(),
            // The TIME axis is off, so the assertion depends on no clock: this proves the COUNT cap
            // ran on its own.
            raw_for_ms: 0,
            max_samples: 10,
            tiers: vec![],
            filter: None,
            // `Policy` derives `Default` precisely so an additive field costs no call-site churn.
            // This literal stopped compiling when `updated_by`/`updated_ms` landed with policy
            // provenance — a pre-existing break that took the whole workspace test run down with
            // it — and the spread is what stops the next field breaking it again.
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(sample_count(&node.store, ws, "fleet.pi").await.unwrap(), 60);

    boot_wiring(&node, ws).await;

    let capped =
        eventually(|| async { sample_count(&node.store, ws, "fleet.pi").await.unwrap() == 10 })
            .await;
    assert!(
        capped,
        "boot must spawn the retention GC: without it a correctly-configured cap is decorative and \
         the series grows until the disc is full"
    );
}

/// BOOT WIRING — the retention **cadence** is config, not a hardcoded const (rubix-ai#84).
///
/// The test above cannot see the period at all: `tokio::time::interval` fires its FIRST tick
/// immediately, so a reactor wired to 300 s and one wired to 1 s are indistinguishable to it. That
/// is exactly why the cadence went unverified — and why proving it needs a **second** tick.
///
/// So: stage a series, boot with a 1 s period, and let the first tick cap it. Then stage MORE rows
/// and commit them, and require the series to shrink again *without anyone calling the verb*. Only a
/// recurring tick can do that, and at the 300 s default it could not happen inside this test's
/// timeout — so the assertion holds only because `retention_period` was honoured.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_retention_cadence_is_configurable_so_a_second_tick_is_observable() {
    let node = Arc::new(Node::boot().await.unwrap());
    let ws = "acme";
    lb_ingest::write(&node.store, ws, &staged("fleet.pi", 60), 0)
        .await
        .expect("stage");
    lb_host::drain_workspace(&node.store, ws)
        .await
        .expect("commit");
    set_policy(
        &node.store,
        ws,
        &Policy {
            prefix: "fleet.".into(),
            raw_for_ms: 0,
            max_samples: 10,
            tiers: vec![],
            filter: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    boot_wiring_every(&node, ws, Some(Duration::from_secs(1))).await;
    assert!(
        eventually(|| async { sample_count(&node.store, ws, "fleet.pi").await.unwrap() == 10 })
            .await,
        "first tick must cap the series"
    );

    // Second round: push the series back OVER the cap. `staged` starts at seq 1, which the committed
    // rows already own, so continue the sequence — a re-used `(series, producer, seq)` would upsert
    // the same rows and grow the count by nothing.
    let more: Vec<Sample> = (0..60)
        .map(|i| Sample {
            series: "fleet.pi".into(),
            producer: "pi-7".into(),
            ts: FIRST_TS_MS + (1_000 + i) * 1_000,
            seq: 1_000 + i,
            payload: json!(i),
            labels: Default::default(),
            qos: Qos::BestEffort,
        })
        .collect();
    lb_ingest::write(&node.store, ws, &more, 0)
        .await
        .expect("stage more");
    lb_host::drain_workspace(&node.store, ws)
        .await
        .expect("commit more");
    assert!(
        sample_count(&node.store, ws, "fleet.pi").await.unwrap() > 10,
        "the series must actually be over the cap again for the second tick to have work to do"
    );

    assert!(
        eventually(|| async { sample_count(&node.store, ws, "fleet.pi").await.unwrap() == 10 })
            .await,
        "a SECOND retention tick must fire on the configured period — at the 300 s default this \
         cannot happen within the timeout, so this is what proves the cadence is honoured rather \
         than hardcoded"
    );
}
