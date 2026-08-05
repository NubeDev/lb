//! The last retention GC pass, persisted (series-observability scope).
//!
//! `run_gc` already computes a pass summary; before this it existed only as a return value that the
//! retention reactor logged and dropped, so "when did GC last run" was an `eprintln!` on the node's
//! stdout and was lost on restart. This module makes it a value a UI can read.
//!
//! **One upserted row per workspace — last pass only, never an append.** A per-pass time series
//! grows without bound, which would be an embarrassing shape for the subsystem whose whole job is
//! bounding growth; at a 300 s reactor cadence an append would add ~10k rows per workspace per
//! year to answer a question with one answer. Pass HISTORY, if ever wanted, is a telemetry concern.
//!
//! **Written by `run_gc`, not by the reactor.** The on-demand `series.retention.gc` verb and the
//! periodic reactor both go through `run_gc`, so recording there gives one path; recording in the
//! reactor would let a manual GC leave the status stale — the status would then lie, which is worse
//! than having no status at all.
//!
//! **A missing row is a valid state**, reported as "no pass recorded on this node" — not an error
//! and not a fabricated zero. No migration is needed: a node that has never run GC has never run
//! GC, and a node that runs no reactors (`BootConfig::reactors` off) will honestly never write one.

use lb_store::{Store, StoreError};
use serde_json::{json, Value};

/// The per-workspace last-GC-pass table. One row, at the fixed id below.
pub const GC_PASS_TABLE: &str = "series_gc_pass";
/// The fixed record id — there is exactly one pass row per workspace namespace.
pub const GC_PASS_ID: &str = "last";

/// How many advisory warnings the stored row keeps. The row is rewritten every reactor tick, so an
/// unpoliced-series warning per series would make a hot, unboundedly-wide row on a deep workspace.
/// `warnings_total` keeps the count honest when the list is clipped.
pub const MAX_STORED_WARNINGS: usize = 20;

/// One recorded GC pass. Mirrors [`crate::GcPass`] plus when it ran and how long it took.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GcPassRecord {
    /// Wall-clock ms at which the pass ran — the caller's `now_ms`, so tests stamp a constant
    /// exactly as `run_gc` does (determinism §3).
    pub last_run_ms: u64,
    /// Wall-clock duration of the pass. Cheap to record and the earliest signal that GC is becoming
    /// expensive on a deep workspace — a pass creeping toward its own 300 s period is the failure
    /// this number catches before it becomes a backlog.
    pub duration_ms: u64,
    pub evicted_raw: usize,
    pub capped_raw: usize,
    pub rollup_rows: usize,
    pub evicted_rollup: usize,
    /// Advisory warnings, clipped to [`MAX_STORED_WARNINGS`].
    #[serde(default)]
    pub warnings: Vec<String>,
    /// How many warnings the pass produced before clipping.
    #[serde(default)]
    pub warnings_total: usize,
    /// Clock skew detected on this pass, in ms — see [`crate::GcPass::clock_skew_ms`].
    ///
    /// Persisted rather than left on the return value because the condition outlives the call that
    /// found it: on the field hardware the clock resets on every power cycle, so by the time an
    /// operator looks, the pass that noticed is long gone. This makes "was this node's last pass
    /// inert?" answerable from the store instead of from whoever happened to be watching stdout.
    ///
    /// `None` on a healthy pass AND on every row written before this field existed. An older row
    /// honestly does not know, and `Option` keeps "we did not check" from reading as "the clock was
    /// fine" — the same distinction `last_pass`'s missing row draws for the pass as a whole.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock_skew_ms: Option<u64>,
}

impl GcPassRecord {
    /// Build a record from a finished pass.
    pub fn new(pass: &crate::GcPass, last_run_ms: u64, duration_ms: u64) -> Self {
        Self {
            last_run_ms,
            duration_ms,
            evicted_raw: pass.evicted_raw,
            capped_raw: pass.capped_raw,
            rollup_rows: pass.rollup_rows,
            evicted_rollup: pass.evicted_rollup,
            warnings: pass
                .warnings
                .iter()
                .take(MAX_STORED_WARNINGS)
                .cloned()
                .collect(),
            warnings_total: pass.warnings.len(),
            clock_skew_ms: pass.clock_skew_ms,
        }
    }
}

/// Record `rec` as THE last pass for `ws`, replacing any previous row.
///
/// Called unconditionally at the end of every pass — **including a pass that evicted nothing.** An
/// idle node that skipped the write would show a frozen `last_run_ms` and read as a dead reactor,
/// turning the observability feature into a false-alarm generator. "GC ran and there was nothing to
/// do" and "GC has not run" are different facts and must stay different.
pub async fn record_pass(store: &Store, ws: &str, rec: &GcPassRecord) -> Result<(), StoreError> {
    store
        .query_ws_retrying(
            ws,
            &format!("UPSERT type::thing('{GC_PASS_TABLE}', $id) CONTENT $row"),
            vec![
                ("id".into(), Value::String(GC_PASS_ID.to_string())),
                ("row".into(), json!(rec)),
            ],
        )
        .await?;
    Ok(())
}

/// The last recorded pass for `ws`, or `None` when this node has never run one.
pub async fn last_pass(store: &Store, ws: &str) -> Result<Option<GcPassRecord>, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!(
                // Every field is projected BY NAME — a field added to `GcPassRecord` but not to this
                // list reads back as its serde default forever, with the row on disc perfectly
                // correct (the closed-struct trap `list_policies` carries the long note about).
                // `clock_skew_ms` is `Option`, so a row predating it arrives as a present NONE and
                // deserializes to `None` correctly — no `none_as_default` dance needed here.
                "SELECT last_run_ms, duration_ms, evicted_raw, capped_raw, rollup_rows, \
                 evicted_rollup, warnings, warnings_total, clock_skew_ms \
                 FROM ONLY type::thing('{GC_PASS_TABLE}', $id)"
            ),
            vec![("id".into(), Value::String(GC_PASS_ID.to_string()))],
        )
        .await?;
    resp.take(0).map_err(|e| StoreError::Decode(e.to_string()))
}
