//! `ingest.write` — authorize, stamp the authenticated producer, then durable-append to staging.
//!
//! **The producer is the authenticated calling principal**, not a producer-declared id (the resolved
//! lean: un-spoofable, already workspace-scoped). We OVERWRITE each sample's `producer` with
//! `principal.sub()` before staging — so the dedup identity `(series, producer, seq)` cannot be
//! forged to collide with or overwrite another producer's stream (ingest scope).

use lb_auth::Principal;
use lb_ingest::{write as stage_write, Sample};
use lb_store::Store;

use super::authorize::authorize_ingest;
use super::error::IngestError;

/// The default staging bound (max staged rows per workspace) — bounded at the cloud end. A real
/// node folds this into config; the slice fixes a sane default (rate-limiting is out of this slice).
pub const DEFAULT_STAGING_BOUND: usize = 100_000;

/// Append `samples` to `ws`'s staging as `principal`. Authorizes `ingest.write` first, then stamps
/// the authenticated producer onto every sample. Returns the count accepted (committed later by the
/// drain worker / `commit_batch`).
pub async fn ingest_write(
    store: &Store,
    principal: &Principal,
    ws: &str,
    samples: Vec<Sample>,
) -> Result<usize, IngestError> {
    authorize_ingest(principal, ws, "ingest.write")?;
    let stamped: Vec<Sample> = samples
        .into_iter()
        .map(|mut s| {
            s.producer = principal.sub().to_string();
            s
        })
        .collect();
    Ok(stage_write(store, ws, &stamped, DEFAULT_STAGING_BOUND).await?)
}
