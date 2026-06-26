//! `ingest.write(Sample[])` — the durable APPEND into staging. The cheap path: each sample is
//! upserted into `ingest_staging` at its composite id `[series, producer, seq]` with **no indexes,
//! no edges, no rollup-view maintenance** — that expensive work is deferred to the batched `series`
//! commit. A burst hits this cheap path; it never storms the indexed series tables (ingest scope).
//!
//! Bounded at the producer/cloud staging end (rule: every overflow control exists at BOTH ends).
//! When staging is at its bound, the overflow policy decides per-sample: best-effort → drop-oldest
//! (the oldest staged row is evicted to make room); must-deliver → dead-letter the incoming sample
//! rather than drop silently. Acceptance is a disk write (staging is durable), not an in-memory op.
//!
//! Raw verb — run AFTER `caps::check` (the host ingest service is the capability chokepoint). The
//! `producer` field of each sample is overwritten by the caller with the authenticated principal
//! before this runs (un-spoofable dedup identity).

use lb_store::{Store, StoreError};
use serde_json::{json, Value};

use crate::overflow::{enforce_bound, OverflowPolicy};
use crate::sample::{Qos, Sample};
use crate::staging::STAGING_TABLE;

/// Append `samples` to workspace `ws`'s staging, honoring `bound` (max staged rows) via the
/// per-sample overflow policy. Returns the number of samples accepted into staging (drop-oldest may
/// evict an older row to admit a new one; a dead-lettered must-deliver sample is still "accepted"
/// into the dead-letter table, counted as handled).
pub async fn write(
    store: &Store,
    ws: &str,
    samples: &[Sample],
    bound: usize,
) -> Result<usize, StoreError> {
    let mut accepted = 0;
    for sample in samples {
        let policy = match sample.qos {
            Qos::BestEffort => OverflowPolicy::DropOldest,
            Qos::MustDeliver => OverflowPolicy::DeadLetter,
        };
        // Bound check FIRST (both-ends rule): make room or divert before the durable append.
        if !enforce_bound(store, ws, bound, policy, sample).await? {
            // must-deliver was dead-lettered instead of admitted to staging; still handled.
            accepted += 1;
            continue;
        }
        append_one(store, ws, sample).await?;
        accepted += 1;
    }
    Ok(accepted)
}

/// Upsert one sample's staging row at its composite id. Idempotent on `[series, producer, seq]` —
/// an offline producer re-appending after reconnect never creates a second staging row.
async fn append_one(store: &Store, ws: &str, sample: &Sample) -> Result<(), StoreError> {
    let staged = json!({ "sample": sample });
    store
        .query_ws(
            ws,
            &format!(
                "UPSERT type::thing('{STAGING_TABLE}', [$series, $producer, $seq]) CONTENT $row"
            ),
            vec![
                ("series".into(), Value::String(sample.series.clone())),
                ("producer".into(), Value::String(sample.producer.clone())),
                ("seq".into(), Value::Number(sample.seq.into())),
                ("row".into(), staged),
            ],
        )
        .await?;
    Ok(())
}
