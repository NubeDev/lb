//! Release or discard a **held** effect — the approval reactor's two guarded transitions
//! (rules-approvals scope). A rule stages a gated effect `Held`; when its `needs:approval` item
//! resolves, the reactor either [`release`]s it (`held → pending`, now deliverable by the relay) or
//! [`discard`]s it (`held → discarded`, terminal, never sent).
//!
//! **Both transitions are guarded on the current status being `Held`** — the load-bearing idempotency
//! bit. A replay (the reactor ticks twice, or a deferred-then-approved item) re-runs the transition
//! and finds the effect already `Pending`/`Discarded`, so it is a no-op: an effect is released
//! **exactly once** (never double-delivered) and a released effect is never clawed back to discarded
//! by a late reject. This mirrors the outbox's never-double-sent guard. Each returns whether it
//! actually transitioned, so the reactor can tally releases without a re-read.
//!
//! Workspace-namespaced (the hard wall §7): `read`/`write` select the namespace from `ws`, so a ws-B
//! reactor pass can physically only release/discard ws-B effects. Raw verbs — the host reactor is the
//! caps chokepoint (the release runs under the reactor's system authority, not a user cap).

use lb_store::{read, write, Store, StoreError};

use super::model::{Effect, EffectStatus};
use super::TABLE;

/// Release held effect `id` in workspace `ws` for delivery (`held → pending`). Guarded: only an
/// effect currently `Held` transitions; anything else (already `Pending`/`Delivered`/`Discarded`, or
/// absent) is a no-op. Returns `true` iff this call performed the transition (so a replay returns
/// `false` and the relay delivers exactly once). Absent effect → `false` (not an error: the reactor
/// scans resolutions, some of which have no held effect).
pub async fn release(store: &Store, ws: &str, id: &str) -> Result<bool, StoreError> {
    transition(store, ws, id, EffectStatus::Pending).await
}

/// Discard held effect `id` in workspace `ws` (`held → discarded`, terminal — the relay never sends
/// it). Guarded exactly like [`release`]: only a currently-`Held` effect transitions; a replay or a
/// reject-after-approve is a no-op. Returns `true` iff this call performed the transition.
pub async fn discard(store: &Store, ws: &str, id: &str) -> Result<bool, StoreError> {
    transition(store, ws, id, EffectStatus::Discarded).await
}

/// The one guarded read-modify-write both transitions share: load `outbox:{id}`, and iff it is
/// currently `Held`, set it to `to` and upsert. Non-`Held` or absent → no write, returns `false`.
async fn transition(
    store: &Store,
    ws: &str,
    id: &str,
    to: EffectStatus,
) -> Result<bool, StoreError> {
    let Some(value) = read(store, ws, TABLE, id).await? else {
        return Ok(false);
    };
    let mut effect: Effect =
        serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?;
    if effect.status != EffectStatus::Held {
        return Ok(false);
    }
    effect.status = to;
    let updated = serde_json::to_value(&effect).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, id, &updated).await?;
    Ok(true)
}
