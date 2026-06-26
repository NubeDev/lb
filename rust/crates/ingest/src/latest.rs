//! `series.latest(series)` — the single newest committed sample of a series. Kept generic: this is
//! "the last value in the sequence", NOT a "device shadow" (no device concept in core, ingest scope).
//! "Newest" is by `seq` (the monotonic ordering key), not wall-clock `ts`.
//!
//! Across producers, the newest is the globally-highest `seq` for the series. Namespace-scoped — a
//! ws-B call can only ever see ws-B's series. Raw verb, run after `caps::check`.

use lb_store::{Store, StoreError};
use serde_json::Value;

use crate::sample::Sample;
use crate::staging::SERIES_TABLE;

/// The newest committed sample of `series` in `ws` (highest `seq`), or `None` if the series has no
/// committed samples in this workspace.
pub async fn latest(store: &Store, ws: &str, series: &str) -> Result<Option<Sample>, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!(
                "SELECT series, producer, seq, ts, payload FROM {SERIES_TABLE} \
                 WHERE series = $series ORDER BY seq DESC LIMIT 1"
            ),
            vec![("series".into(), Value::String(series.to_string()))],
        )
        .await?;
    let rows: Vec<Sample> = resp.take(0).map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(rows.into_iter().next())
}
