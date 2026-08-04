//! `spawn_store_compact_reactors` — the background driver that (a) drains enqueued
//! `store-compact` jobs off the request path, (b) logs the threshold advisory when the commit log
//! outgrows the node's threshold, and (c) — **only when a disk budget is configured** — enqueues
//! a pass of its own past the soft mark ([`BudgetDriver`], disk-budget scope slice 2).
//!
//! **Threshold-driven, never compaction-on-a-tick** (scope: a periodic pass would trade
//! unbounded disk for a periodic I/O storm — the dev-node-cpu lesson). The tick itself is
//! cheap: one indexed pending-jobs query per workspace + one file-metadata stat. Unbudgeted, a
//! pass runs ONLY when an authorized caller enqueued one and over-threshold logs the advisory and
//! nothing else — today's behaviour, unchanged. Budgeted, the driver may enqueue **one** job in
//! the node's configured workspace (never a fan-out: the pass is node-global and each one
//! quiesces every write on the node), rate-limited by [`AUTO_COMPACT_MIN_INTERVAL`] and held off at
//! the soft mark once passes stop reclaiming (see [`super::budget`]) — but **never held off at the
//! hard mark**, where a suspended driver still retries on [`SUSPENDED_HARD_RETRY_INTERVAL`].
//! Suspending absolutely deadlocked: only an executed pass lifts the suspension and the suspension
//! is what stops one being enqueued (rubix-ai#84).
//!
//! Ticks never overlap (`MissedTickBehavior::Skip`); errors are logged, never fatal. The
//! reactor mints no principal — the capability gate ran at `store.compact` enqueue time, the
//! same posture as `spawn_retention_reactors` executing durable retention policy.

use std::sync::Arc;
use std::time::Duration;

use lb_jobs::{Job, JobStatus};
use lb_store::CompactionRecord;

use crate::boot::Node;
use crate::store_admin::budget::{
    BudgetAction, BudgetDriver, BUDGET_REQUESTED_BY, SUSPENDED_HARD_RETRY_INTERVAL,
};
use crate::store_admin::compact::{CompactJobPayload, STORE_COMPACT_JOB_KIND};
use crate::store_admin::marks::budget_marks;
use crate::store_admin::status::over_threshold_advisory;

/// The drain/advisory cadence. Fast enough that an operator's enqueue starts promptly; the
/// tick does no heavy work by itself.
pub const STORE_COMPACT_PERIOD: Duration = Duration::from_secs(30);

/// Spawn the detached compact-job drain + log-size advisory for `workspaces`. Returns
/// immediately; the loop runs for the life of the node.
///
/// `budget_bytes` is the node's configured disk allowance (`BootConfig::store_budget_bytes`,
/// from `LB_STORE_MAX_BYTES`). `None` ⇒ today's flat advisory and no automatic pass, ever —
/// the budget is config, not a code branch (rule 1). `budget_ws` is the single workspace a
/// budget-driven job is written to (decision 8); a crossing produces exactly one job.
pub fn spawn_store_compact_reactors(
    node: Arc<Node>,
    workspaces: Vec<String>,
    period: Duration,
    budget_bytes: Option<u64>,
    budget_ws: String,
) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(period);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut driver = BudgetDriver::new(budget_marks(budget_bytes));
        // Re-seed the convergence suspension from the PERSISTED last pass (boot-memory-guard scope
        // slice 3). Without this, every restart begins believing compaction still pays here and
        // re-enqueues a pass that a previous run already proved reclaims nothing — a recurring
        // write pause for zero bytes, on exactly the node that restarts most.
        if let Some(rec) = lb_store::last_persisted_compaction(&node.store) {
            driver.note_pass(&rec);
            if driver.is_suspended() {
                tracing::info!(
                    before_bytes = rec.before_bytes,
                    after_bytes = rec.after_bytes,
                    "store budget driver starts suspended: the last persisted pass reclaimed \
                     almost nothing, so automatic passes stay held off until one pays"
                );
            }
        }
        loop {
            ticker.tick().await;
            // The advisory: visible before painful. One stat per tick, warn only while over.
            let snap = lb_store::status(&node.store);
            if let Some(warning) =
                over_threshold_advisory(snap.log_bytes, driver.marks().threshold_bytes)
            {
                tracing::warn!(log_bytes = snap.log_bytes, "{warning}");
            }
            budget_tick(&node, &mut driver, snap.log_bytes, &budget_ws).await;
            for ws in &workspaces {
                match drain_compact_jobs(&node, ws).await {
                    // Fold every pass's outcome back into the driver, whoever asked for it: an
                    // operator's productive pass is what lifts a convergence suspension.
                    Ok(records) => records.iter().for_each(|r| driver.note_pass(r)),
                    Err(e) => tracing::warn!(ws = %ws, error = %e, "store-compact drain failed"),
                }
            }
        }
    });
}

/// One budget decision, and the enqueue it may imply. Split out of the loop so the reactor body
/// stays readable and the enqueue path is reachable from a test without spawning the task.
pub async fn budget_tick(
    node: &Arc<Node>,
    driver: &mut BudgetDriver,
    log_bytes: u64,
    budget_ws: &str,
) {
    let now = std::time::Instant::now();
    // Read the suspension BEFORE the enqueue: `note_enqueued` does not clear it, but the flag is
    // what distinguishes an ordinary hard-mark pass from the suspended retry, and reading it up
    // front keeps that independent of any future bookkeeping in the enqueue arm.
    let suspended = driver.is_suspended();
    match driver.decide(log_bytes, now) {
        BudgetAction::Idle => {}
        BudgetAction::BudgetTooSmall => {
            tracing::warn!(
                log_bytes,
                budget_bytes = ?driver.marks().budget_bytes,
                hard_mark_bytes = ?driver.marks().hard_mark_bytes,
                "store is over the soft mark but compaction reclaims almost nothing — budget too \
                 small for this workload; holding off (raise LB_STORE_MAX_BYTES or tighten \
                 retention). Auto-passes resume once a pass reclaims again, and the node still \
                 retries periodically past the hard mark."
            );
        }
        BudgetAction::Enqueue { hard_mark } => {
            match enqueue_budget_compaction(node, budget_ws).await {
                Ok(job_id) => {
                    driver.note_enqueued(now);
                    if hard_mark && suspended {
                        // The rubix-ai#84 path: the driver believes compaction is not paying here,
                        // and is trying anyway because the alternative is a guaranteed breach. Say
                        // so explicitly — a pass that runs while the previous line said "budget too
                        // small" looks like a contradiction unless the retry is named.
                        tracing::warn!(
                            log_bytes, %job_id,
                            hard_mark_bytes = ?driver.marks().hard_mark_bytes,
                            retry_interval_s = SUSPENDED_HARD_RETRY_INTERVAL.as_secs(),
                            "store is past the HARD disk mark and the last pass reclaimed almost \
                             nothing — retrying anyway, because only a compaction frees bytes on an \
                             append-only engine and declining guarantees the breach. If this repeats \
                             without shrinking the store, the budget really is too small for the \
                             workload: raise LB_STORE_MAX_BYTES or tighten retention."
                        );
                    } else if hard_mark {
                        tracing::warn!(
                            log_bytes, %job_id,
                            hard_mark_bytes = ?driver.marks().hard_mark_bytes,
                            "store crossed the HARD disk mark — compacting now, exempt from the \
                             minimum interval (on an append-only engine only a compaction frees \
                             bytes). Writes pause for the pass."
                        );
                    } else {
                        tracing::info!(
                            log_bytes, %job_id,
                            soft_mark_bytes = ?driver.marks().soft_mark_bytes,
                            "store crossed the disk soft mark — enqueued an automatic store.compact"
                        );
                    }
                }
                // Decision 8: if the configured workspace is somehow absent, log and skip rather
                // than guessing at another one.
                Err(e) => {
                    tracing::warn!(ws = %budget_ws, error = %e, "budget-driven store.compact enqueue failed — skipping")
                }
            }
        }
    }
}

/// Write the budget driver's own `store-compact` job. No principal is minted and no capability is
/// checked: this is node maintenance below the namespace wall, the same posture as the retention
/// reactor executing durable policy. `requested_by` names the driver, not a person.
async fn enqueue_budget_compaction(
    node: &Arc<Node>,
    ws: &str,
) -> Result<String, lb_store::StoreError> {
    let job_id = format!("store-compact-{}", lb_store::new_ulid());
    let payload = serde_json::to_string(&CompactJobPayload {
        requested_by: BUDGET_REQUESTED_BY.to_string(),
        outcome: None,
        error: None,
    })
    .unwrap_or_default();
    let job = Job::new(
        job_id.clone(),
        STORE_COMPACT_JOB_KIND,
        payload,
        now_wall_ms(),
    );
    lb_jobs::create(&node.store, ws, &job).await?;
    Ok(job_id)
}

/// Wall-clock now in epoch ms — the reactor's own clock, for the job record's stamp.
fn now_wall_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Run every pending `store-compact` job in `ws`: execute the pass, record the outcome on the
/// job record, complete it. Sequential — passes must never overlap (each quiesces all writes).
///
/// Returns the record of every pass that succeeded, so the budget driver can judge whether
/// compaction is still paying here (the convergence condition) from *any* pass, not just its own.
pub async fn drain_compact_jobs(
    node: &Arc<Node>,
    ws: &str,
) -> Result<Vec<CompactionRecord>, lb_store::StoreError> {
    let pending = lb_jobs::pending(&node.store, ws, STORE_COMPACT_JOB_KIND).await?;
    let mut records = Vec::new();
    for mut job in pending {
        let started = std::time::Instant::now();
        let result = lb_store::compact(&node.store).await;
        let mut payload: CompactJobPayload =
            serde_json::from_str(&job.payload).unwrap_or(CompactJobPayload {
                requested_by: String::new(),
                outcome: None,
                error: None,
            });
        let status = match result {
            Ok(rec) => {
                tracing::info!(
                    ws = %ws,
                    job = %job.id,
                    before_bytes = rec.before_bytes,
                    after_bytes = rec.after_bytes,
                    duration_ms = rec.duration_ms,
                    "store compaction pass complete"
                );
                records.push(rec.clone());
                payload.outcome = Some(rec);
                JobStatus::Done
            }
            Err(e) => {
                tracing::warn!(ws = %ws, job = %job.id, error = %e, "store compaction pass failed");
                payload.error = Some(e.to_string());
                JobStatus::Failed
            }
        };
        job.payload = serde_json::to_string(&payload).unwrap_or_default();
        lb_jobs::create(&node.store, ws, &job).await?; // upsert the outcome onto the record
        lb_jobs::complete(&node.store, ws, &job.id, status).await?;
        tracing::debug!(ws = %ws, job = %job.id, elapsed_ms = started.elapsed().as_millis() as u64, "store-compact job drained");
    }
    Ok(records)
}
