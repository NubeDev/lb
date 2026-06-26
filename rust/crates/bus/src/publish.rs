//! Publish a message onto a workspace-scoped bus key (motion, §3.3).
//!
//! The caller passes a workspace-relative key (`chan/general/msg`); `ws_key` prepends the
//! `ws/{id}/` prefix so the published key is `ws/{id}/chan/general/msg`. A peer in another
//! workspace cannot name that key without the prefix the host controls — the workspace wall
//! is structural on the bus (README §7). Durability is NOT here: the store keeps the record,
//! the bus only moves it (§3.3); this verb is fire-and-forget.

use crate::key::ws_key;
use crate::peer::{Bus, BusError};

/// Publish `payload` onto `(ws, rel)`. The payload is opaque bytes (the channel service
/// passes serialized item JSON). Returns once Zenoh has accepted it for delivery.
pub async fn publish(bus: &Bus, ws: &str, rel: &str, payload: &[u8]) -> Result<(), BusError> {
    let key = ws_key(ws, rel);
    bus.session()
        .put(&key, payload.to_vec())
        .await
        .map_err(|e| BusError::Session(e.to_string()))
}
