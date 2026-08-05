//! The disk-budget **driver**: decide, on each store-compact reactor tick, whether the node
//! should enqueue an automatic `store.compact` pass (disk-budget scope, issue #122, slice 2).
//!
//! This is the deferral in `online-compaction-scope.md` OQ5 being reversed, so it is deliberately
//! conservative. Three guards stand between a budget crossing and a write pause:
//!
//! 1. **A budget must be configured.** Unbudgeted (`BootConfig::store_budget_bytes == None`) ⇒ no
//!    marks, no decision, today's warn-only behaviour byte-for-byte (decision 1/2).
//! 2. **A minimum interval** between auto-passes ([`AUTO_COMPACT_MIN_INTERVAL`], one hour) — the
//!    hard mark is **exempt** from it (decision 5). The exemption is not a tuning knob: on an
//!    append-only engine a delete *adds* bytes and only a compaction frees them, so a reclamation
//!    path that can be blocked by an interval is a path that can blow the budget.
//! 3. **A convergence condition.** When a pass returns `after_bytes > 0.9 x before_bytes`
//!    ([`PRODUCTIVE_RECLAIM_RATIO`]) the *live set* is the budget — compaction is not the problem,
//!    and re-enqueuing every eligible tick is a recurring write outage for zero reclaimed bytes.
//!    The driver stops auto-enqueueing and logs "budget too small for this workload" **at the soft
//!    mark**, and resumes on its own the next time any pass (operator- or budget-triggered) pays.
//!    It is **not absolute**: past the hard mark a suspended driver still retries, rate-limited to
//!    [`SUSPENDED_HARD_RETRY_INTERVAL`]. An absolute suspension deadlocked — only an executed pass
//!    clears it, and the suspension is what stops one being enqueued — and that deadlock is how a
//!    store blew its budget and grew unbounded with its compaction count frozen (rubix-ai#84).
//!
//! The decision itself is pure — no store, no clock, no I/O — so the convergence regression (no
//! second job over many ticks) is testable without seeding a gigabyte.

use std::time::{Duration, Instant};

use lb_store::CompactionRecord;
// The convergence judgement now lives in `lb_store::boot_guard` so the BOOT precondition and this
// runtime driver make the same call from ONE definition (boot-memory-guard scope slice 1) — two
// copies of "did this pass pay?" is exactly the drift that makes a skip and an enqueue disagree.
pub use lb_store::{is_productive, PRODUCTIVE_RECLAIM_RATIO};

use super::marks::BudgetMarks;

/// Minimum wall time between *automatic* passes (decision 5). A hard-mark crossing is exempt.
pub const AUTO_COMPACT_MIN_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// How often a **suspended** driver retries at the hard mark (rubix-ai#84).
///
/// The convergence suspension used to be absolute, which deadlocked: only an executed pass clears
/// it and the suspension is what blocks one. But retrying every tick is a permanent write outage on
/// a store whose live set really is the budget — the compact reactor ticks every 30 s. Five minutes
/// is the compromise, and the two sides size it:
///
/// - **Fast enough to matter.** At the measured ~6.6 MB/min (100 points) the soft→hard span is
///   ~2.9 min and hard→budget ~1 min, so a retry lands inside the window where reclaiming anything
///   still prevents a breach. At the 1800-point target those windows are shorter still, but a
///   *bounded* retry that sometimes arrives late is strictly better than one that never comes.
/// - **Slow enough not to be an outage.** A pass measured 2.9–4.0 s on a ~100 MB store (7–23 s on
///   RC-6 storage), so one pass per 5 min is ~1% duty cycle at worst — a real cost, paid only while
///   the node is over 95% of its budget and already in trouble.
///
/// This is deliberately NOT reusing [`AUTO_COMPACT_MIN_INTERVAL`]: an hour is chosen to stop
/// *pointless* soft-mark churn, and a store past the hard mark that waits an hour to try again has
/// already blown the budget many times over at any realistic ingest rate.
pub const SUSPENDED_HARD_RETRY_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// `requested_by` on a budget-driven job — deliberately not a real principal, so an operator
/// reading the job record sees at a glance that the budget driver caused the pause (decision 8).
pub const BUDGET_REQUESTED_BY: &str = "system:store-budget";

/// What the driver decided this tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetAction {
    /// Under the soft mark, unbudgeted, or held off by the minimum interval: do nothing.
    Idle,
    /// Enqueue one `store.compact` job in the node's configured workspace.
    Enqueue { hard_mark: bool },
    /// Over the soft mark, but compaction has stopped paying. Log; never enqueue.
    BudgetTooSmall,
}

/// The driver's per-node state. Lives in the reactor task for the life of the node; nothing here
/// is durable (rule 4 — an interval and a flag are motion, not state).
#[derive(Debug, Clone)]
pub struct BudgetDriver {
    marks: BudgetMarks,
    /// When the last automatic pass was enqueued; `None` until the first one.
    last_auto_at: Option<Instant>,
    /// Set when a pass reclaimed essentially nothing: auto-enqueueing is suspended until one pays.
    unproductive: bool,
    /// When the last *suspended* hard-mark retry was enqueued; `None` until the first one. Separate
    /// from `last_auto_at` because the two intervals are different lengths and answer different
    /// questions (see [`SUSPENDED_HARD_RETRY_INTERVAL`]).
    last_suspended_retry_at: Option<Instant>,
}

impl BudgetDriver {
    pub fn new(marks: BudgetMarks) -> Self {
        Self {
            marks,
            last_auto_at: None,
            unproductive: false,
            last_suspended_retry_at: None,
        }
    }

    pub fn marks(&self) -> BudgetMarks {
        self.marks
    }

    /// Decide from the current log size. `now` is injected so the interval is testable without
    /// sleeping an hour.
    pub fn decide(&self, log_bytes: u64, now: Instant) -> BudgetAction {
        // Unbudgeted ⇒ no marks ⇒ the driver is inert. This is the whole "upgrade changes
        // nothing" property.
        let (Some(soft), Some(hard)) = (self.marks.soft_mark_bytes, self.marks.hard_mark_bytes)
        else {
            return BudgetAction::Idle;
        };
        if log_bytes < soft {
            return BudgetAction::Idle;
        }
        let hard_mark = log_bytes >= hard;

        // A SUSPENDED DRIVER STILL RETRIES AT THE HARD MARK — on its own slow interval.
        //
        // The old ordering checked the suspension first and so returned `BudgetTooSmall` at 95%, at
        // 100%, and at any size above. That was the AC 1 failure (rubix-ai#84): a store sailed past
        // its 120 MB budget and grew unbounded with its compaction count frozen at 4, while the log
        // repeated "budget too small" once a minute. Two things were wrong with it:
        //
        //  1. **It deadlocks.** Only an executed pass clears the latch (`note_pass`), and the latch
        //     is what prevents a pass being enqueued. Nothing inside the node can break the cycle —
        //     it takes an operator noticing and compacting by hand.
        //  2. **The verdict is least reliable exactly when it is consulted.** A one-shot ratio
        //     cannot tell "the live set IS the budget" from "the live set has not started shrinking
        //     yet". On a fresh deployment retention evicts nothing until data passes `raw_for_ms`,
        //     so every early pass measures a monotonically growing live set and reads as
        //     unproductive — true of that instant, false of the steady state minutes later. The
        //     live breach happened ~4 minutes BEFORE the 30-minute raw horizon was first reachable,
        //     i.e. before retention could free a single byte.
        //
        // But the suspension guards something real, and retrying on EVERY tick would trade a
        // silent breach for a permanent write outage: the compact reactor ticks every 30 s, so a
        // store whose live set genuinely is the budget would compact twice a minute forever. The
        // resolution is that a suspended retry is rate-limited by its own
        // [`SUSPENDED_HARD_RETRY_INTERVAL`] rather than being either free or forbidden. One pass
        // per interval is a bounded, self-correcting cost: if the live set really is the budget the
        // pass reclaims little and `note_pass` re-arms the suspension; if retention has since made
        // rows deletable, the pass pays and the suspension lifts on its own.
        //
        // Below the hard mark nothing changes: a suspended driver holds off at the soft mark and
        // says the useful thing, which is the write-outage the suspension exists to prevent.
        if self.unproductive {
            return if hard_mark && self.suspended_retry_due(now) {
                BudgetAction::Enqueue { hard_mark: true }
            } else {
                BudgetAction::BudgetTooSmall
            };
        }
        if hard_mark || self.interval_elapsed(now) {
            return BudgetAction::Enqueue { hard_mark };
        }
        BudgetAction::Idle
    }

    fn interval_elapsed(&self, now: Instant) -> bool {
        match self.last_auto_at {
            None => true,
            Some(prev) => now.saturating_duration_since(prev) >= AUTO_COMPACT_MIN_INTERVAL,
        }
    }

    /// Is a suspended hard-mark retry due? Tracked on its own stamp rather than `last_auto_at`, so
    /// the ordinary hourly interval and the 5-minute suspended retry cannot mask each other.
    fn suspended_retry_due(&self, now: Instant) -> bool {
        match self.last_suspended_retry_at {
            None => true,
            Some(prev) => now.saturating_duration_since(prev) >= SUSPENDED_HARD_RETRY_INTERVAL,
        }
    }

    /// Record that an automatic pass was enqueued at `now` (starts the minimum interval). While
    /// suspended this also stamps the hard-mark retry clock — the caller does not have to know
    /// which kind of enqueue it just made, so the two cannot drift apart.
    pub fn note_enqueued(&mut self, now: Instant) {
        self.last_auto_at = Some(now);
        if self.unproductive {
            self.last_suspended_retry_at = Some(now);
        }
    }

    /// Fold in the outcome of a pass — **any** pass the node ran, whichever principal asked for
    /// it. A productive one clears the suspension; an unproductive one sets it.
    pub fn note_pass(&mut self, rec: &CompactionRecord) {
        if !rec.ok || rec.skipped.is_some() {
            // A failed pass — or one a boot precondition declined to run — says nothing about
            // whether compaction pays here. Concluding "unproductive" from a skip would suspend the
            // runtime driver precisely on the RAM-bound node that most needs it to keep working.
            return;
        }
        self.unproductive = !is_productive(rec.before_bytes, rec.after_bytes);
    }

    /// True while auto-enqueueing is suspended by the convergence condition.
    pub fn is_suspended(&self) -> bool {
        self.unproductive
    }
}
