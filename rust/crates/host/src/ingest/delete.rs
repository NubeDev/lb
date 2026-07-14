//! `series.delete` — the host's capability chokepoint for removing a whole series (data-console
//! scope: series lifecycle). Gates first (`mcp:series.delete:call`, workspace-first §3.6 then
//! capability §3.5), then calls the raw `lb_ingest::delete_series`, which clears the series' samples,
//! rollups, staged rows, registry row, and tag edges in `ws`.
//!
//! A denial is opaque [`IngestError::Denied`] — no existence signal. Destructive, so it carries its
//! own cap (not folded into `ingest.write`): the privilege to *destroy* a series is distinct from the
//! privilege to write into it.

use lb_auth::Principal;
use lb_ingest::delete_series;
use lb_store::Store;

use super::authorize::authorize_ingest;
use super::error::IngestError;

/// Delete `series` and its whole footprint in `ws`, as `principal`. Authorizes
/// `mcp:series.delete:call` first. Idempotent — deleting an unknown series succeeds (no-op).
pub async fn series_delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    series: &str,
) -> Result<(), IngestError> {
    authorize_ingest(principal, ws, "series.delete")?;
    Ok(delete_series(store, ws, series).await?)
}
