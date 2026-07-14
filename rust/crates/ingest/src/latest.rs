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
