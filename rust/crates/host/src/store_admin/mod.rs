//! The store's operational surface — two host-native MCP verbs over the embedded engine's
//! commit log (online-compaction scope, issue #67):
//!
//!   - `store.status() -> { persistent, log_bytes, segment_count, last_compaction, threshold_bytes,
//!     advisory }` ([`store_status_run`]) — the cheap observability read (file metadata only,
//!     below the namespace wall — no records touched). Gated `store:status:read`.
//!   - `store.compact() -> { job_id }` ([`store_compact_enqueue`]) — **a job, never inline**:
//!     compaction is whole-log I/O with no upper bound, so the verb enqueues and returns; the
//!     reactor ([`spawn_store_compact_reactors`]) drains it off the request path. Gated
//!     `store:compact:run` (admin — running a pass pauses every writer behind the session mutex).
//!
//! The reactor also carries the threshold advisory (past the node's threshold it logs the
//! visibility-first warning) and, **when a disk budget is configured**, the budget driver that
//! auto-enqueues a pass past the soft mark ([`budget`], disk-budget scope slice 2 — OQ5's
//! deferral, reversed by a measured 771 ms pause on a 2.06 GiB log). Unbudgeted, nothing
//! auto-triggers and the behaviour is exactly what release 1 shipped.

mod authorize;
mod budget;
mod compact;
mod error;
mod marks;
mod reactor;
mod status;
mod tool;

pub use authorize::{authorize_store_compact, authorize_store_status};
pub use budget::{
    is_productive, BudgetAction, BudgetDriver, AUTO_COMPACT_MIN_INTERVAL, BUDGET_REQUESTED_BY,
    PRODUCTIVE_RECLAIM_RATIO, SUSPENDED_HARD_RETRY_INTERVAL,
};
pub use compact::{store_compact_enqueue, STORE_COMPACT_JOB_KIND};
pub use error::StoreAdminError;
pub use marks::{budget_marks, BudgetMarks, HARD_MARK_PCT, SOFT_MARK_PCT};
pub use reactor::{
    budget_tick, drain_compact_jobs, spawn_store_compact_reactors, STORE_COMPACT_PERIOD,
};
pub use status::{
    over_threshold_advisory, store_status_run, store_status_run_with_budget, StoreStatusReport,
    LOG_ADVISORY_BYTES,
};
pub use tool::call_store_admin_tool;
