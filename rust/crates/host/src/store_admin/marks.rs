//! The disk-budget mark arithmetic: `Option<u64>` budget → the advisory threshold and the soft/hard
//! marks (disk-budget scope, decision 1). Pure — no store, no I/O, no clock — so both the status
//! verb and (slice 2) the reactor derive the same numbers from the same one place, and the
//! "`None` ⇒ exactly today's behaviour" property is unit-testable without booting a node.

use super::status::LOG_ADVISORY_BYTES;

/// The soft mark, as a percentage of the budget: reclaim what we already know how to reclaim.
pub const SOFT_MARK_PCT: u64 = 80;

/// The hard mark, as a percentage of the budget: compact (exempt from the minimum interval) and
/// log loudly. It never refuses a write (scope decision 3).
pub const HARD_MARK_PCT: u64 = 95;

/// The thresholds a node runs with, derived from its configured budget.
///
/// Two shapes, and only two:
///   - **No budget** (`budget_bytes: None`) ⇒ `threshold_bytes == LOG_ADVISORY_BYTES` and **no
///     marks at all**. This is today's behaviour byte-for-byte: a flat 256 MiB advisory that warns
///     and nothing more. Tying the marks to the budget's *existence* is what makes slice 1 purely
///     additive (decision 1) and what keeps an upgrade from silently acquiring new behaviour
///     (decision 2).
///   - **A budget** ⇒ the marks are percentages of it, and the advisory threshold *is* the soft
///     mark, so the operator is told at the same point the node starts acting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetMarks {
    /// The configured allowance, echoed back; `None` ⇒ unbudgeted.
    pub budget_bytes: Option<u64>,
    /// The byte count this node warns past — the soft mark when budgeted, else the flat advisory.
    pub threshold_bytes: u64,
    /// 80% of the budget; `None` when unbudgeted.
    pub soft_mark_bytes: Option<u64>,
    /// 95% of the budget; `None` when unbudgeted.
    pub hard_mark_bytes: Option<u64>,
}

impl BudgetMarks {
    /// Bytes left before the budget is spent, given the store's current size. `None` when
    /// unbudgeted (there is no ceiling to have headroom against). Saturating: a store already over
    /// budget reports `0`, never an underflow.
    pub fn headroom_bytes(&self, log_bytes: u64) -> Option<u64> {
        self.budget_bytes.map(|b| b.saturating_sub(log_bytes))
    }
}

/// Derive the thresholds from an optional budget. The one place the percentages are applied.
pub fn budget_marks(budget_bytes: Option<u64>) -> BudgetMarks {
    match budget_bytes {
        // Unbudgeted: today's flat advisory, and nothing to trigger on.
        None => BudgetMarks {
            budget_bytes: None,
            threshold_bytes: LOG_ADVISORY_BYTES,
            soft_mark_bytes: None,
            hard_mark_bytes: None,
        },
        Some(budget) => {
            let soft = pct_of(budget, SOFT_MARK_PCT);
            BudgetMarks {
                budget_bytes: Some(budget),
                threshold_bytes: soft,
                soft_mark_bytes: Some(soft),
                hard_mark_bytes: Some(pct_of(budget, HARD_MARK_PCT)),
            }
        }
    }
}

/// `pct`% of `budget`, without overflowing a multi-exabyte allowance: divide first when the
/// multiply would wrap. Truncating (a mark is a floor, so it fires no later than intended).
fn pct_of(budget: u64, pct: u64) -> u64 {
    match budget.checked_mul(pct) {
        Some(p) => p / 100,
        None => (budget / 100) * pct,
    }
}
