//! **`BootConfig::retention_seed`** — the boot-provisioned retention policy (rubix-ai#84).
//!
//! The defect these pin is a REBUILD, not a fresh install: a node whose bounds had been applied by
//! hand and proven live was rebuilt and came back on the stock keep-for-ever policy, silently. So the
//! shape of every test here is *run the seeder more than once, and against a store someone else has
//! already written to* — a test that only asserts "a fresh store gets a policy" cannot see the bug.
//!
//! Real node, real store (`mem://`), real `series.retention` rows through the real `lb_ingest`
//! verbs. No mocks (testing §0).

use std::sync::Arc;

use lb_ingest::{list_policies, set_policy, Policy, Tier};
use lb_node::seed_retention::SEEDED_BY;
use lb_node::{BootConfig, Node};

/// A bounded seed policy: a raw window AND a count cap, with a rollup tier bounded both ways. This
/// is the shape `docs/scope/ingest/rollup-row-cap-scope.md` and rubix-ai's `disc-failsafe-scope.md`
/// converge on — `keep_for_ms` is the interim bound, `max_rows` the clock-free one that supersedes
/// it on a box with no working clock.
fn bounded(prefix: &str) -> Policy {
    Policy {
        prefix: prefix.into(),
        raw_for_ms: 30 * 60 * 1_000,
        max_samples: 120,
        tiers: vec![Tier {
            width_ms: 15 * 60 * 1_000,
            keep_for_ms: 7 * 24 * 60 * 60 * 1_000,
            max_rows: 672,
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// `BootConfig` is `#[non_exhaustive]`, so it is built by mutation rather than a struct literal —
/// which is the point of the attribute: adding a field must not break an embedder's construction.
fn cfg_with(ws: &str, seed: Vec<Policy>) -> BootConfig {
    let mut cfg = BootConfig::default();
    cfg.workspace = ws.into();
    cfg.retention_seed = seed;
    cfg
}

/// A fresh store — the rebuilt node — comes up bounded, with nobody calling `series.retention.set`.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_fresh_store_comes_up_bounded_with_no_operator_action() {
    let node = Arc::new(Node::boot().await.unwrap());
    let cfg = cfg_with("nube", vec![bounded("plant.")]);

    lb_node::seed_retention::run(&node, &cfg).await;

    let got = list_policies(&node.store, "nube").await.unwrap();
    assert_eq!(
        got.len(),
        1,
        "exactly one policy row, no shadowing duplicate"
    );
    let p = &got[0];
    assert_eq!(p.prefix, "plant.");
    assert_eq!(p.max_samples, 120);
    let t = &p.tiers[0];
    assert!(
        t.keep_for_ms != 0 || t.max_rows != 0,
        "the seeded tier must be bounded — a tier with neither bound IS the stock policy this \
         feature exists to stop a rebuilt node reverting to"
    );
    assert_eq!(
        p.updated_by.as_deref(),
        Some(SEEDED_BY),
        "provenance says the seed stood it — 'operator or re-provisioning?' was the question \
         rubix-ai#84 could not answer about RC-6"
    );
}

/// **The AC 8/9 invariant.** An operator's policy — here a deliberate keep-for-ever one, the case
/// most easily mistaken for "unset, fix it" — survives a restart untouched, and does NOT earn a
/// second, shadowing row.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_operator_policy_is_never_stomped_across_restarts() {
    let node = Arc::new(Node::boot().await.unwrap());
    let operator = Policy {
        prefix: "plant.".into(),
        raw_for_ms: 3_600_000,
        max_samples: 0,
        tiers: vec![Tier {
            width_ms: 60_000,
            keep_for_ms: 0, // deliberate: this site keeps its rollups for ever
            max_rows: 0,
            ..Default::default()
        }],
        updated_by: Some("user:test".into()),
        ..Default::default()
    };
    set_policy(&node.store, "nube", &operator).await.unwrap();

    let cfg = cfg_with("nube", vec![bounded("plant.")]);
    // Three boots: the seeder must be idempotent, not just first-run-correct.
    for _ in 0..3 {
        lb_node::seed_retention::run(&node, &cfg).await;
    }

    let got = list_policies(&node.store, "nube").await.unwrap();
    assert_eq!(got.len(), 1, "no per-prefix shadow row — one policy, still");
    assert_eq!(got[0], operator, "the operator's row is byte-identical");
}

/// Seeding is per prefix: an occupied prefix is left alone while an absent sibling is still
/// provisioned. Without this, one operator override would silently disable every other seed.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn seeding_is_per_prefix_not_all_or_nothing() {
    let node = Arc::new(Node::boot().await.unwrap());
    let operator = Policy {
        prefix: "plant.".into(),
        raw_for_ms: 999,
        updated_by: Some("user:test".into()),
        ..Default::default()
    };
    set_policy(&node.store, "nube", &operator).await.unwrap();

    let cfg = cfg_with("nube", vec![bounded("plant."), bounded("depot.")]);
    lb_node::seed_retention::run(&node, &cfg).await;

    let got = list_policies(&node.store, "nube").await.unwrap();
    assert_eq!(got.len(), 2);
    let plant = got.iter().find(|p| p.prefix == "plant.").unwrap();
    let depot = got.iter().find(|p| p.prefix == "depot.").unwrap();
    assert_eq!(plant.raw_for_ms, 999, "the operator's prefix untouched");
    assert_eq!(depot.updated_by.as_deref(), Some(SEEDED_BY));
}

/// An empty seed changes nothing at all — the default, and what the standalone binary passes. Boot
/// must be byte-identical to before this field existed.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_empty_seed_writes_nothing() {
    let node = Arc::new(Node::boot().await.unwrap());
    lb_node::seed_retention::run(&node, &cfg_with("nube", vec![])).await;
    assert!(list_policies(&node.store, "nube").await.unwrap().is_empty());
}

/// The seed is workspace-scoped like every other series-plane record: seeding `nube` must not put a
/// row in another workspace's namespace.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_seed_respects_the_workspace_wall() {
    let node = Arc::new(Node::boot().await.unwrap());
    lb_node::seed_retention::run(&node, &cfg_with("nube", vec![bounded("plant.")])).await;

    assert_eq!(list_policies(&node.store, "nube").await.unwrap().len(), 1);
    assert!(list_policies(&node.store, "other")
        .await
        .unwrap()
        .is_empty());
}
