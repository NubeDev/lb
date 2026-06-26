//! The durable, append-only staging table — the buffer's backing store. A producer's samples land
//! here *cheaply* (single table, no secondary indexes, no rollup-view maintenance, no tag-graph
//! edges); the expensive indexed/edged work happens at the batched `series` commit. This is the
//! cheap-append-vs-expensive-indexed-write relief the buffer exists to provide (ingest scope).
//!
//! Staging rows are keyed by the same composite `[series, producer, seq]` as the committed sample,
//! so an offline producer re-appending after a reconnect is idempotent at the staging layer too —
//! never two staging rows for one logical sample. The commit worker drains rows from here.

use serde::{Deserialize, Serialize};

use crate::sample::Sample;

/// The staging table — one per workspace namespace (the hard wall holds for the firehose path
/// exactly as for channels).
pub const STAGING_TABLE: &str = "ingest_staging";

/// The committed time-series table. A sample record lives at `series:[series, producer, seq]`.
pub const SERIES_TABLE: &str = "series";

/// The dead-letter table for must-deliver samples that overflow the bound (ingest scope: overflow
/// is dead-letter for must-deliver, drop-oldest for best-effort).
pub const DEAD_LETTER_TABLE: &str = "ingest_dead_letter";

/// A staged sample as stored — the `Sample` plus nothing else (staging is a pure landing zone).
/// Kept as its own struct so the staging shape can evolve independently of the wire `Sample`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Staged {
    pub sample: Sample,
}
