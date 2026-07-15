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
//! The reactor also carries the threshold advisory: past `LOG_ADVISORY_BYTES` it logs the same
//! visibility-first warning posture as the sample-cap over-cap warnings — it never auto-triggers
//! a pass (scope OQ5: operator-triggered for release 1, auto once the pause cost is measured in
//! the field).

mod authorize;
mod compact;
mod error;
mod reactor;
mod status;
mod tool;

pub use authorize::{authorize_store_compact, authorize_store_status};
pub use compact::{store_compact_enqueue, STORE_COMPACT_JOB_KIND};
pub use error::StoreAdminError;
pub use reactor::{drain_compact_jobs, spawn_store_compact_reactors, STORE_COMPACT_PERIOD};
pub use status::{
    over_threshold_advisory, store_status_run, StoreStatusReport, LOG_ADVISORY_BYTES,
};
pub use tool::call_store_admin_tool;
