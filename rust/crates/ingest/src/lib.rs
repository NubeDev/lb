//! `lb-ingest` — a generic buffered read/write surface for high-volume external data (ingest scope,
//! README §6.1 time-series). The **read-side analog of the outbox**: the outbox guarantees
//! must-deliver effects *out*; ingest absorbs high-volume data *in*, through the same
//! capability-gated MCP contract as everything else.
//!
//! **This is NOT an IoT system.** A "device" is just a principal on a node; the surface is a generic
//! `series` of timestamped values. No `device`/`sensor`/`firmware`/`MQTT` concept appears anywhere in
//! this crate — protocol adapters are out-of-core extensions that normalize raw bytes to `Sample[]`.
//!
//! The shape (one verb per file, FILE-LAYOUT):
//!   - [`Sample`] — the canonical envelope; dedup identity is `(series, producer, seq)`.
//!   - [`write`] — durable APPEND into staging (the cheap path; no indexes/edges on that write).
//!   - [`commit_direct`] — commit a live batch straight to `series`, skipping the staging
//!     round-trip (one log append per sample instead of three).
//!   - [`commit_batch`] — drain a batch and commit it in ONE transaction, UPSERT on
//!     `[series, producer, seq]` (atomic + exactly-once on re-drain).
//!   - [`read`] / [`latest`] — range query / newest value over the committed series.
//!   - [`enforce_bound`] — the overflow policy (drop-oldest / dead-letter), bounded at both ends.
//!
//! Authorization is NOT here — these are raw verbs run after `caps::check` (the host ingest service
//! is the capability chokepoint, capability-first §3.5). Engine is config (`Store::open` vs
//! `memory()`), never a role branch.

mod align;
mod bucket;
mod bucket_acc;
mod cap;
mod clock_sanity;
mod commit;
mod cursor;
mod dead_letter_gc;
mod delete;
mod direct;
mod filter;
mod filter_pass;
mod filter_state;
mod gc;
mod labels;
mod latest;
mod meta;
mod method;
mod overflow;
mod page;
mod pass_record;
mod read;
mod rename;
mod retention;
mod rollup;
mod rollup_cap;
mod rollup_window;
mod sample;
mod samples_delete;
mod samples_update;
mod schema;
mod staging;
mod stats;
mod write;

pub use align::{bucket_start, Align};
pub use bucket::{
    effective_width, read_buckets, read_buckets_fold, Bucket, BucketQuery, Source, MAX_BUCKETS,
};
pub use cap::{cap_series, default_cap_notice, sample_count, CAP_EVICT_BATCH, DEFAULT_MAX_SAMPLES};
pub use clock_sanity::{
    backwards_warning, clock_went_backwards, newest_sample_ms, skew, skew_warning,
    SKEW_TOLERANCE_MS,
};
pub use commit::{commit_batch, commit_batch_capped, commit_staged, CommitPass, Dequeue};
pub use cursor::Cursor;
pub use dead_letter_gc::{prune_dead_letters, DEAD_LETTER_KEEP_MS};
pub use delete::delete_series;
pub use direct::{commit_direct, commit_direct_capped, DIRECT_COMMIT_BATCH};
pub use filter::{
    decide, Deadband, Decision, Filter, FilterCounts, LastCommitted, Range, RangeMode, Reason,
};
pub use filter_state::{read_filter_state, ProducerState, FILTER_STATE_FIELD};
pub use gc::{run_gc, GcPass};
pub use latest::{latest, latest_many};
pub use meta::{series_names, DEFAULT_SERIES_CAP};
pub use method::{apply_method, Method};
pub use overflow::{enforce_bound, staged_count, OverflowPolicy};
pub use page::{
    read_page, Direction, Page, PageError, PageQuery, DEFAULT_PAGE_LIMIT, MAX_PAGE_LIMIT,
};
pub use pass_record::{
    last_pass, record_pass, GcPassRecord, GC_PASS_ID, GC_PASS_TABLE, MAX_STORED_WARNINGS,
};
pub use read::read;
pub use rename::{rename_series, RenameError};
pub use retention::{
    delete_policy, list_policies, resolve_policy, set_policy, Policy, Tier, RETENTION_TABLE,
};
pub use rollup::{read_rollups, rollup_widths, write_rollups, RollupRow};
pub use rollup_cap::{cap_rollup_rows, rollup_count};
pub use sample::{Qos, Sample};
pub use samples_delete::{delete_samples_by_keys, delete_samples_in_range, SampleKey};
pub use samples_update::{update_samples, SampleUpdate};
pub use schema::{ensure_series_schema, ROLLUP_TABLE, SERIES_META_TABLE};
pub use staging::{DEAD_LETTER_TABLE, SERIES_TABLE, STAGING_TABLE};
pub use stats::{series_producers, series_stats, SeriesStats, TierRows};
pub use write::write;
