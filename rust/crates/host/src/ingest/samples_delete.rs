//! `series.samples.delete` — the host's capability chokepoint for bulk-deleting committed raw
//! samples (series lifecycle). Gates first (`mcp:series.samples.delete:call`, workspace-first §3.6
//! then capability §3.5), then calls the raw `lb_ingest` verb. Admin-only cap: removing other
//! producers' committed rows is workspace-data administration, like `series.delete`/`rename`.
//!
//! Exactly ONE selector mode per call — either explicit `keys` or a `from`/`to` seq range (at
//! least one bound). A call with neither is `BadInput`: "delete everything" must stay the explicit
//! `series.delete`, never an accidentally-empty selector. Rolled-up history is immutable here —
//! only the raw tail is touched.

use lb_auth::Principal;
use lb_ingest::{delete_samples_by_keys, delete_samples_in_range, SampleKey};
use lb_store::Store;

use super::authorize::authorize_ingest;
use super::error::IngestError;

/// Delete committed samples of `series` in `ws`, as `principal`. Authorizes
/// `mcp:series.samples.delete:call` first. Returns the number of rows actually removed (a key or
/// range matching nothing removes 0 — a no-op, not an error).
pub async fn series_samples_delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    series: &str,
    keys: Option<Vec<SampleKey>>,
    from_seq: Option<u64>,
    to_seq: Option<u64>,
) -> Result<usize, IngestError> {
    authorize_ingest(principal, ws, "series.samples.delete")?;
    let has_range = from_seq.is_some() || to_seq.is_some();
    match (keys, has_range) {
        (Some(_), true) => Err(IngestError::BadInput(
            "pick one selector: keys OR a from/to seq range, not both".into(),
        )),
        (Some(keys), false) if keys.is_empty() => Err(IngestError::BadInput(
            "keys must be a non-empty array of {producer, seq}".into(),
        )),
        (Some(keys), false) => Ok(delete_samples_by_keys(store, ws, series, &keys).await?),
        (None, true) => Ok(delete_samples_in_range(store, ws, series, from_seq, to_seq).await?),
        (None, false) => Err(IngestError::BadInput(
            "missing selector: provide keys or at least one of from/to \
             (deleting a whole series is series.delete)"
                .into(),
        )),
    }
}
