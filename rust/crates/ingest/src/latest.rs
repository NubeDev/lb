//! `series.latest(series)` — the single newest committed sample of a series. Kept generic: this is
//! "the last value in the sequence", NOT a "device shadow" (no device concept in core, ingest scope).
//!
//! **"Newest" is by `ts`, tie-broken by `seq`.** `seq` is monotonic per `(series, producer)` ONLY —
//! it is a *within-stream* ordering key and carries no meaning across producers, so comparing two
//! producers' seqs is comparing two unrelated scales. Ordering the whole series by `seq DESC` did
//! exactly that: a producer whose in-memory `seq` restarts at 0 (any restarted process) re-entered
//! the series below the previous stream's high-water mark, and `latest` pinned to the OLD stream's
//! last sample forever while fresh data landed at lower seqs and never surfaced. `ts` is the only
//! axis the streams share.
//!
//! `ts` is the producer's clock and may skew — but a skewed clock is a *data* problem visible to the
//! caller, whereas the seq ordering was a *correctness* problem invisible to everyone. Within one
//! producer `seq DESC` still breaks ties, so a producer batching many samples into one `ts` keeps
//! its exact intra-batch order.
//!
//! Namespace-scoped — a ws-B call can only ever see ws-B's series. Raw verb, run after `caps::check`.

use lb_store::{Store, StoreError};
use serde_json::Value;

use crate::sample::Sample;
use crate::staging::SERIES_TABLE;

/// The newest committed sample of `series` in `ws` (latest `ts`, tie-broken by highest `seq`), or
/// `None` if the series has no committed samples in this workspace.
pub async fn latest(store: &Store, ws: &str, series: &str) -> Result<Option<Sample>, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!(
                "SELECT series, producer, seq, time::millis(ts) AS ts, payload FROM {SERIES_TABLE} \
                 WHERE series = $series ORDER BY ts DESC, seq DESC LIMIT 1"
            ),
            vec![("series".into(), Value::String(series.to_string()))],
        )
        .await?;
    let rows: Vec<Sample> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(rows.into_iter().next())
}

/// The newest committed sample of EACH requested series in `ws`, in one query — the fleet-snapshot
/// batch of [`latest`] (series-read-perf scope). Collapses a K-series "now" view from K authorize +
/// query round-trips into one: a single `WHERE series IN $names` scan, ordered newest-first
/// (`ts DESC, seq DESC` — the same "newest" as [`latest`], for the same restart-safety reason), from
/// which the first row seen per series is that series' latest.
///
/// Returns an entry for **every** requested name (a series with no committed samples → `None`), so
/// the caller does no reconciliation — parity with single [`latest`]'s null-not-error contract.
/// Namespace-scoped: a ws-B call resolves ws-A names to nothing (every one `None`). Raw verb, run
/// after `caps::check`.
pub async fn latest_many(
    store: &Store,
    ws: &str,
    names: &[String],
) -> Result<Vec<(String, Option<Sample>)>, StoreError> {
    // Pre-seed every requested name to None so absent series still appear (dedup preserves order).
    let mut out: Vec<(String, Option<Sample>)> = Vec::with_capacity(names.len());
    for n in names {
        if !out.iter().any(|(k, _)| k == n) {
            out.push((n.clone(), None));
        }
    }
    if out.is_empty() {
        return Ok(out);
    }

    // Newest-first across the whole name list; the FIRST row seen for a series is its latest.
    let mut resp = store
        .query_ws(
            ws,
            &format!(
                "SELECT series, producer, seq, time::millis(ts) AS ts, payload FROM {SERIES_TABLE} \
                 WHERE series IN $names ORDER BY ts DESC, seq DESC"
            ),
            vec![(
                "names".into(),
                Value::Array(names.iter().map(|n| Value::String(n.clone())).collect()),
            )],
        )
        .await?;
    let rows: Vec<Sample> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;

    for s in rows {
        if let Some(slot) = out.iter_mut().find(|(k, v)| k == &s.series && v.is_none()) {
            slot.1 = Some(s);
        }
    }
    Ok(out)
}
