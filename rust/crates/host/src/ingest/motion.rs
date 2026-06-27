//! Series **motion** — the live feed a dashboard widget watches (dashboard scope, "Bus (Zenoh)").
//! State vs motion (rule 3): the committed `series` table is state (`series.read`/`series.latest`);
//! a freshly written sample is *also* published onto the workspace-scoped bus subject
//! `ws/{id}/series/{series}` so a widget sees it advance **without polling**. Fire-and-forget,
//! best-effort — a dropped live frame is fine; the durable copy is the committed series.
//!
//! `publish_sample` is called by the write path (the gateway's `POST /ingest` after staging+drain);
//! `subscribe_series` is the live read, gated by `mcp:series.read:call` (workspace-first), backing
//! the gateway's `GET /series/{series}/stream` SSE route — the series analog of the channel stream.

use lb_auth::Principal;
use lb_bus::{publish, subscribe, Bus, Subscription};

use lb_ingest::Sample;

use super::authorize::authorize_ingest;
use super::error::IngestError;

/// The workspace-relative bus key a series' motion rides (`series/{series}`); `ws_key` prepends the
/// `ws/{id}/` wall so a subscriber cannot express interest in another workspace's series (§7).
fn series_key(series: &str) -> String {
    format!("series/{series}")
}

/// Publish `sample` onto its series' motion subject in `ws`. Best-effort (motion, §3.3) — a failure
/// to publish never fails the durable write; the committed series is the truth. The payload is the
/// serialized `Sample` (the same shape `series.read` returns), so a widget folds it in directly.
pub async fn publish_sample(bus: &Bus, ws: &str, sample: &Sample) -> Result<(), IngestError> {
    let payload =
        serde_json::to_vec(sample).map_err(|e| IngestError::BadInput(format!("sample: {e}")))?;
    publish(bus, ws, &series_key(&sample.series), &payload)
        .await
        .map_err(|e| IngestError::BadInput(e.to_string()))?;
    Ok(())
}

/// A live subscription to one series' samples. Wraps the bus subscription and deserializes each
/// payload back into the `Sample` the series speaks (a malformed frame is skipped, never stalls).
pub struct SeriesSub {
    inner: Subscription,
}

impl SeriesSub {
    /// Await the next live sample. `None` once the subscription closes.
    pub async fn recv(&self) -> Option<Sample> {
        loop {
            let bytes = self.inner.recv().await?;
            match serde_json::from_slice::<Sample>(&bytes) {
                Ok(s) => return Some(s),
                Err(_) => continue,
            }
        }
    }
}

/// Subscribe to live samples on `series` in `ws` as `principal`. Authorizes `series.read` FIRST
/// (workspace-first), so a denied or cross-workspace caller never even declares bus interest — the
/// `403` the SSE route returns before any stream opens.
pub async fn subscribe_series(
    bus: &Bus,
    principal: &Principal,
    ws: &str,
    series: &str,
) -> Result<SeriesSub, IngestError> {
    authorize_ingest(principal, ws, "series.read")?;
    let inner = subscribe(bus, ws, &series_key(series))
        .await
        .map_err(|e| IngestError::BadInput(e.to_string()))?;
    Ok(SeriesSub { inner })
}
