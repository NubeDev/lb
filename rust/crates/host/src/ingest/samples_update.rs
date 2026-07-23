//! `series.samples.update` — the host's capability chokepoint for editing committed raw samples in
//! place (series lifecycle). Gates first (`mcp:series.samples.update:call`, workspace-first §3.6
//! then capability §3.5), then calls the raw `lb_ingest::update_samples`. Admin-only cap: editing
//! other producers' committed rows is workspace-data administration, like `series.delete`/`rename`.
//!
//! UPDATE semantics, never UPSERT: an update naming a non-existent sample is skipped — it cannot
//! create a row, and in particular cannot plant one under a foreign producer identity. Each entry
//! must set at least one of `payload`/`ts`, else `BadInput`. Rolled-up history is immutable here.

use lb_auth::Principal;
use lb_ingest::{update_samples, SampleUpdate};
use lb_store::Store;

use super::authorize::authorize_ingest;
use super::error::IngestError;

/// Apply `updates` to committed samples of `series` in `ws`, as `principal`. Authorizes
/// `mcp:series.samples.update:call` first. Returns the number of rows actually updated (an entry
/// naming a missing sample counts 0 — skipped, never created).
pub async fn series_samples_update(
    store: &Store,
    principal: &Principal,
    ws: &str,
    series: &str,
    updates: Vec<SampleUpdate>,
) -> Result<usize, IngestError> {
    authorize_ingest(principal, ws, "series.samples.update")?;
    if updates.is_empty() {
        return Err(IngestError::BadInput(
            "updates must be a non-empty array of {producer, seq, payload?, ts?}".into(),
        ));
    }
    if let Some(empty) = updates
        .iter()
        .find(|u| u.payload.is_none() && u.ts.is_none())
    {
        return Err(IngestError::BadInput(format!(
            "update for producer '{}' seq {} sets neither payload nor ts",
            empty.producer, empty.seq
        )));
    }
    Ok(update_samples(store, ws, series, &updates).await?)
}
