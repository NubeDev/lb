//! `series.read` / `series.latest` — authorize, then read the committed series. Both are
//! namespace-scoped through the store, so a ws-B reader can physically only see ws-B's series
//! (the hard wall) and a denied caller learns nothing (ingest scope).

use lb_auth::Principal;
use lb_ingest::{latest as series_latest, read as series_read, Sample};
use lb_store::Store;

use super::authorize::authorize_ingest;
use super::error::IngestError;

/// Range query over the committed `series` in `ws`: samples with `seq` in `[from_seq, to_seq]`,
/// ordered by `seq`. Gated by `mcp:series.read:call`.
pub async fn series_read_range(
    store: &Store,
    principal: &Principal,
    ws: &str,
    series: &str,
    from_seq: Option<u64>,
    to_seq: Option<u64>,
) -> Result<Vec<Sample>, IngestError> {
    authorize_ingest(principal, ws, "series.read")?;
    Ok(series_read(store, ws, series, from_seq, to_seq).await?)
}

/// The newest committed sample of `series` in `ws` (highest `seq`), or `None`. Gated by
/// `mcp:series.latest:call`. Generic "last value" — not a device shadow.
pub async fn series_latest_value(
    store: &Store,
    principal: &Principal,
    ws: &str,
    series: &str,
) -> Result<Option<Sample>, IngestError> {
    authorize_ingest(principal, ws, "series.latest")?;
    Ok(series_latest(store, ws, series).await?)
}
