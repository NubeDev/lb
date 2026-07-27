//! `series.stats` — the capability-gated read of what one series holds (series-observability scope).
//!
//! A **data-plane** read: sample counts, wall-clock extent, and the producer set. Gated by
//! `mcp:series.stats:call`, granted with the other `series.*` reads (the data-console tier) rather
//! than with retention administration — a caller allowed to read samples is allowed to know how
//! many there are, and keeping this cap off the admin tier is what lets a UI degrade PER FACT
//! (counts and freshness stay visible when retention bookkeeping is refused).
//!
//! Single-subject; see [`lb_ingest::series_stats`] for why there is deliberately no array mode.
//!
//! A denial is an `IngestError::Denied` → `ToolError::Denied`, which is a REFUSAL, not an empty
//! success. A series with no samples returns a valid all-zero [`SeriesStats`]. Those two states must
//! never collapse into each other: the whole point of this verb is letting a caller tell "not
//! permitted" from "nothing here".

use lb_auth::Principal;
use lb_ingest::{series_stats, SeriesStats};
use lb_store::Store;

use super::authorize::authorize_ingest;
use super::error::IngestError;

/// Statistics for one series in `ws`. Gated by `mcp:series.stats:call`.
pub async fn series_stats_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    series: &str,
) -> Result<SeriesStats, IngestError> {
    authorize_ingest(principal, ws, "series.stats")?;
    Ok(series_stats(store, ws, series).await?)
}
