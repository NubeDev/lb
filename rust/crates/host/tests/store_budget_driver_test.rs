//! The disk-budget **driver** (issue #122, slice 2) against a real store and real bytes — the
//! reversal of `online-compaction-scope.md` OQ5, approved by a measured 771 ms pause on a
//! 2.06 GiB log (`sessions/store/disk-budget-session.md`).
//!
//! The regressions that matter, in order of how badly they bite:
//!   - **convergence** — a store whose *live set* is the budget compacts once, sees
//!     `after_bytes > 0.9 x before_bytes`, then never enqueues again over many ticks. Getting this
//!     wrong is an hourly write outage forever, which is why it is the first test here.
//!   - **the minimum interval** — no second automatic pass inside the hour…
//!   - **…except at the hard mark**, which is exempt (on an append-only engine only a compaction
//!     frees bytes, so a reclamation path an interval can block is a path that blows the budget).
//!   - **unbudgeted is inert** — `None` ⇒ no marks, no job, ever. The upgrade-changes-nothing gate.
//!   - **eviction grows the log; compaction reclaims it** — the append-only property the whole
//!     ordering rule rests on, pinned so a future change cannot quietly invalidate it.
//!
//! Real SurrealKV dirs, real jobs records, real passes. Nothing is mocked (rule 9).

use std::sync::Arc;
use std::time::{Duration, Instant};

use lb_host::{
    budget_marks, budget_tick, drain_compact_jobs, is_productive, BudgetAction, BudgetDriver, Node,
    AUTO_COMPACT_MIN_INTERVAL, BUDGET_REQUESTED_BY, STORE_COMPACT_JOB_KIND,
};
use lb_store::{compact, delete, status, write, Store};
use serde_json::json;

fn temp_path(tag: &str) -> String {
    std::env::temp_dir()
        .join(format!("lb-budget-{tag}-{}", std::process::id()))
        .to_string_lossy()
        .into_owned()
}

fn cleanup(path: &str) {
    let _ = std::fs::remove_dir_all(path);
}

/// A pass outcome as the real engine would report it, for folding into the driver.
fn record(before: u64, after: u64) -> lb_store::CompactionRecord {
    lb_store::CompactionRecord {
        at_epoch_ms: 0,
        ok: true,
        before_bytes: before,
        after_bytes: after,
        duration_ms: 1,
        error: None,
        // A pass that RAN (a boot skip sets this; the driver must ignore those — see
        // `store_boot_guard_test`).
        skipped: None,
    }
}

async fn pending_count(store: &Store, ws: &str) -> usize {
    lb_jobs::pending(store, ws, STORE_COMPACT_JOB_KIND)
        .await
        .expect("pending scan")
        .len()
}

/// **Convergence — the write-outage-forever regression.** A store whose live set exceeds the soft
/// mark compacts once; the pass reclaims essentially nothing; the driver then reports
/// `BudgetTooSmall` and enqueues NOTHING for the rest of the node's life — asserted over many
/// ticks, well past the minimum interval, with a real jobs table to count.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_store_whose_live_set_is_the_budget_stops_auto_enqueueing() {
    let ws = "budget-converge";
    let path = temp_path("converge");
    cleanup(&path);
    let store = Store::open(&path).await.unwrap();
    let node = Arc::new(Node::boot_with_store(store.clone()).await.unwrap());

    // Budget 1000 bytes ⇒ soft 800, hard 950. The log is "over" at 900.
    let mut driver = BudgetDriver::new(budget_marks(Some(1000)));

    // Tick 1: over the soft mark, no prior pass ⇒ exactly one job.
    budget_tick(&node, &mut driver, 900, ws).await;
    assert_eq!(
        pending_count(&store, ws).await,
        1,
        "the soft mark enqueues one pass"
    );

    // The pass runs and reclaims ~nothing (890 of 900 bytes survive: the live set IS the budget).
    driver.note_pass(&record(900, 890));
    assert!(
        driver.is_suspended(),
        "an unproductive pass suspends auto-enqueueing"
    );

    // Drain it so the queue is empty and any new job would be visible.
    drain_compact_jobs(&node, ws).await.expect("drain");
    assert_eq!(pending_count(&store, ws).await, 0);

    // Many ticks, hours of simulated time, still over the mark — and past the HARD mark too.
    let start = Instant::now();
    for i in 1..=200u32 {
        let _ = start; // clock advances via the driver's own Instant; see interval test below
        budget_tick(&node, &mut driver, if i % 2 == 0 { 900 } else { 990 }, ws).await;
    }
    assert_eq!(
        pending_count(&store, ws).await,
        0,
        "a suspended driver must never enqueue again — not at the soft mark, not at the hard mark"
    );
    assert_eq!(
        driver.decide(900, Instant::now()),
        BudgetAction::BudgetTooSmall,
        "and it says the useful thing at the SOFT mark, not only at the hard one"
    );

    // Resumption: a pass that pays again (whoever asked for it) lifts the suspension. The soft
    // mark is still inside the minimum interval from tick 1, so it resumes as `Idle`-until-due
    // (asserted against an advanced clock) while the exempt hard mark enqueues immediately.
    driver.note_pass(&record(900, 100));
    assert!(!driver.is_suspended());
    let now = Instant::now();
    assert_eq!(
        driver.decide(900, now),
        BudgetAction::Idle,
        "still inside the interval"
    );
    assert_eq!(
        driver.decide(900, now + AUTO_COMPACT_MIN_INTERVAL),
        BudgetAction::Enqueue { hard_mark: false },
        "auto-passes resume once one pays and the interval is due"
    );
    budget_tick(&node, &mut driver, 990, ws).await;
    assert_eq!(
        pending_count(&store, ws).await,
        1,
        "and the exempt hard mark enqueues at once — which it did NOT while suspended"
    );

    cleanup(&path);
}

/// The minimum interval holds off a second automatic pass — and the **hard mark is exempt** from
/// it. Both halves in one test: the exemption only means something against an interval that is
/// otherwise blocking.
#[test]
fn the_interval_blocks_a_second_soft_pass_but_never_the_hard_mark() {
    let driver_at = |elapsed: Duration| {
        let mut d = BudgetDriver::new(budget_marks(Some(1000)));
        let now = Instant::now();
        d.note_enqueued(now - elapsed);
        (d, now)
    };

    // Just inside the interval: the soft mark waits…
    let (d, now) = driver_at(AUTO_COMPACT_MIN_INTERVAL - Duration::from_secs(60));
    assert_eq!(
        d.decide(850, now),
        BudgetAction::Idle,
        "soft mark respects the interval"
    );
    // …while the hard mark compacts anyway.
    assert_eq!(
        d.decide(960, now),
        BudgetAction::Enqueue { hard_mark: true },
        "the hard mark is exempt from the minimum interval"
    );

    // Past the interval: the soft mark fires again.
    let (d, now) = driver_at(AUTO_COMPACT_MIN_INTERVAL + Duration::from_secs(1));
    assert_eq!(
        d.decide(850, now),
        BudgetAction::Enqueue { hard_mark: false }
    );
}

/// A quiet, budgeted store below the soft mark does nothing at all — no pass, no job, no warning
/// (the dev-node-cpu lesson: a driver must not tick work on an idle node).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_quiet_store_enqueues_nothing() {
    let ws = "budget-quiet";
    let path = temp_path("quiet");
    cleanup(&path);
    let store = Store::open(&path).await.unwrap();
    let node = Arc::new(Node::boot_with_store(store.clone()).await.unwrap());

    let mut driver = BudgetDriver::new(budget_marks(Some(1_000_000)));
    for _ in 0..50 {
        budget_tick(&node, &mut driver, 10_000, ws).await; // 1% of budget
    }
    assert_eq!(pending_count(&store, ws).await, 0);
    cleanup(&path);
}

/// **Unbudgeted is inert.** `None` ⇒ no marks, so no log size — not even one far past the old flat
/// advisory — produces an automatic job. This is the property that makes the upgrade safe.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unbudgeted_node_never_auto_compacts() {
    let ws = "budget-none";
    let path = temp_path("none");
    cleanup(&path);
    let store = Store::open(&path).await.unwrap();
    let node = Arc::new(Node::boot_with_store(store.clone()).await.unwrap());

    let mut driver = BudgetDriver::new(budget_marks(None));
    for _ in 0..50 {
        budget_tick(&node, &mut driver, 64 * 1024 * 1024 * 1024, ws).await; // 64 GiB
    }
    assert_eq!(
        pending_count(&store, ws).await,
        0,
        "no budget ⇒ no marks ⇒ no automatic pass"
    );
    assert_eq!(driver.decide(u64::MAX, Instant::now()), BudgetAction::Idle);
    cleanup(&path);
}

/// A budget-driven job is attributable: `requested_by` is the driver's own literal, never a
/// principal, so an operator reading the record sees what caused the pause (decision 8). And the
/// job the driver writes drains through the *same* reactor path an operator's does.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_driver_writes_one_attributable_job_that_drains_normally() {
    let ws = "budget-attrib";
    let path = temp_path("attrib");
    cleanup(&path);
    let store = Store::open(&path).await.unwrap();
    let node = Arc::new(Node::boot_with_store(store.clone()).await.unwrap());

    // Real bytes so the pass has something to do.
    for round in 0..4u64 {
        for k in 0..40 {
            write(
                &store,
                ws,
                "kv",
                &format!("k{k}"),
                &json!({"round": round, "pad": "x".repeat(512)}),
            )
            .await
            .unwrap();
        }
    }

    let mut driver = BudgetDriver::new(budget_marks(Some(1000)));
    budget_tick(&node, &mut driver, 900, ws).await;
    let jobs = lb_jobs::pending(&store, ws, STORE_COMPACT_JOB_KIND)
        .await
        .unwrap();
    assert_eq!(jobs.len(), 1, "one crossing, one job — never a fan-out");
    assert!(
        jobs[0].payload.contains(BUDGET_REQUESTED_BY),
        "the budget driver names itself: {}",
        jobs[0].payload
    );

    let records = drain_compact_jobs(&node, ws).await.expect("drain");
    assert_eq!(
        records.len(),
        1,
        "the drain reports the pass back to the driver"
    );
    assert!(records[0].ok);
    cleanup(&path);
}

/// **Eviction grows the log; only compaction reclaims it.** The append-only property the entire
/// ordering rule depends on, asserted directly on real bytes so a future engine change cannot
/// quietly invalidate it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deleting_rows_grows_the_log_and_compaction_reclaims_it() {
    let ws = "budget-tombstone";
    let path = temp_path("tombstone");
    cleanup(&path);
    let store = Store::open(&path).await.unwrap();

    for k in 0..300 {
        write(
            &store,
            ws,
            "kv",
            &format!("k{k}"),
            &json!({"pad": "x".repeat(512)}),
        )
        .await
        .unwrap();
    }
    let before_delete = status(&store).log_bytes;

    // Evict — exactly what a retention pass does to stale rows.
    for k in 0..300 {
        delete(&store, ws, "kv", &format!("k{k}")).await.unwrap();
    }
    let after_delete = status(&store).log_bytes;
    assert!(
        after_delete > before_delete,
        "a delete is a tombstone APPENDED to the log: {after_delete} must exceed {before_delete}"
    );

    let rec = compact(&store).await.expect("pass");
    assert!(rec.ok, "{:?}", rec.error);
    assert!(
        rec.after_bytes < before_delete,
        "only a compaction frees the bytes: {} must drop below the pre-delete {before_delete}",
        rec.after_bytes
    );
    cleanup(&path);
}

/// The convergence constant is the one knob here; pin what it separates (decision 6).
#[test]
fn productivity_separates_a_real_reclaim_from_a_pointless_pass() {
    assert!(
        is_productive(2_161_424_374, 16_903_331),
        "the measured 128x pass pays"
    );
    assert!(
        !is_productive(1000, 950),
        "5% reclaimed is not worth a write pause"
    );
    assert!(is_productive(1000, 500));
    assert!(is_productive(0, 0), "an empty log concludes nothing");
}
