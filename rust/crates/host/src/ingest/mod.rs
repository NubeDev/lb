//! The ingest service — the host's capability chokepoint for the generic buffered read/write
//! surface (ingest scope, README §6.1). Wraps the raw `lb_ingest` verbs with the gate (capability-
//! first, §3.5; isolation-first, §3.6) and stamps the authenticated producer onto every sample.
//!
//! The verbs (one per file, FILE-LAYOUT):
//!   - `ingest.write` ([`ingest_write`]) — authorize, stamp producer, durable-append to staging.
//!   - `series.read` / `series.latest` ([`series_read_range`]/[`series_latest_value`]) — read the
//!     committed series.
//!   - the **commit worker** ([`drain_workspace`]) — drains staging → series in one tx per batch,
//!     driven by [`spawn_ingest_reactors`] and mounted by the **ingest role** (config, not a code
//!     branch). A CALLER never uses it: a request drains [`drain_workspace_bounded`] to its own
//!     batch, so one producer's write latency cannot scale with another's backlog
//!     (drain-backpressure scope).
//!   - the MCP bridge ([`call_ingest_tool`]) — the one MCP contract over all of the above.
//!
//! NOT an IoT system: no device/sensor/firmware/MQTT concept anywhere here — a producer is a
//! principal, the surface is a generic `series`.

mod authorize;
mod delete;
mod drain;
mod drain_reactor;
mod error;
mod find;
mod list;
mod motion;
mod read;
mod rename;
mod retention;
mod tool;
mod write;

pub use authorize::authorize_ingest;
pub use delete::series_delete;
pub use drain::{drain_workspace, drain_workspace_bounded, own_batches, DrainPass, COMMIT_BATCH};
pub use drain_reactor::spawn_ingest_reactors;
pub use error::IngestError;
pub use find::series_find;
pub use list::{series_list, MAX_SERIES_LIST};
pub use motion::{publish_sample, subscribe_series, SeriesSub};
pub use read::{series_latest_value, series_read_buckets, series_read_page, series_read_range};
pub use rename::series_rename;
pub use retention::{
    series_retention_delete, series_retention_gc, series_retention_list, series_retention_set,
};
pub use tool::call_ingest_tool;
pub use write::{ingest_write, DEFAULT_STAGING_BOUND};

// Re-export the wire envelope so host callers / tests use one `Sample`/`Qos` type.
pub use lb_ingest::{Qos, Sample};
