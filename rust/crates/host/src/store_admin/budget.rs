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
//!
//! The decision itself is pure — no store, no clock, no I/O — so the convergence regression (no
//! second job over many ticks) is testable without seeding a gigabyte.

use std::time::{Duration, Instant};

use lb_store::CompactionRecord;

use super::marks::BudgetMarks;

/// Minimum wall time between *automatic* passes (decision 5). A hard-mark crossing is exempt.
pub const AUTO_COMPACT_MIN_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// A pass that leaves more than this fraction of the log behind reclaimed essentially nothing.
/// Any value in 0.85..0.95 separates that from the measured 26-65x bloat case; named so tuning it
/// later is a one-line change with a test (decision 6).
pub const PRODUCTIVE_RECLAIM_RATIO: f64 = 0.9;

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
}

impl BudgetDriver {
    pub fn new(marks: BudgetMarks) -> Self {
        Self {
            marks,
            last_auto_at: None,
            unproductive: false,
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
        // Compaction has stopped paying: say the useful thing at the soft mark, and keep saying it
        // past the hard mark. A pass that reclaims nothing at 80% reclaims nothing at 95% — the
        // hard mark's interval exemption exists to beat the *clock*, not this.
        if self.unproductive {
            return BudgetAction::BudgetTooSmall;
        }
        let hard_mark = log_bytes >= hard;
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

    /// Record that an automatic pass was enqueued at `now` (starts the minimum interval).
    pub fn note_enqueued(&mut self, now: Instant) {
        self.last_auto_at = Some(now);
    }

    /// Fold in the outcome of a pass — **any** pass the node ran, whichever principal asked for
    /// it. A productive one clears the suspension; an unproductive one sets it.
    pub fn note_pass(&mut self, rec: &CompactionRecord) {
        if !rec.ok {
            return; // a failed pass says nothing about whether compaction pays here
        }
        self.unproductive = !is_productive(rec.before_bytes, rec.after_bytes);
    }

    /// True while auto-enqueueing is suspended by the convergence condition.
    pub fn is_suspended(&self) -> bool {
        self.unproductive
    }
}

/// Did the pass reclaim enough to be worth the write pause? A pass over an empty log is
/// productive by convention (there was nothing to reclaim, so nothing to conclude).
pub fn is_productive(before_bytes: u64, after_bytes: u64) -> bool {
    if before_bytes == 0 {
        return true;
    }
    (after_bytes as f64) <= PRODUCTIVE_RECLAIM_RATIO * (before_bytes as f64)
}
