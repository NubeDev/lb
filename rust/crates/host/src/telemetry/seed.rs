//! `telemetry_seed` — write ONE real telemetry row into a workspace's capped ring **and** mirror it
//! onto the ws-walled tail subject, through the exact production write path (telemetry-console scope,
//! testing §3.1). This is the helper the test-gateway `/_seed/telemetry` route calls so a UI
//! gateway test can plant real rows the console reads back over `telemetry.query`/`tail` — it is
//! **seeding, not faking** (CLAUDE §9): it performs the same `capped_insert` + tail publish the
//! `SurrealCappedLayer` performs on a real dispatch event, just driven from the test instead of from a
//! live `tracing` event (the test gateway installs no global subscriber).
//!
//! Redaction holds here too: the caller passes already-digested params (`params_digest`), never a raw
//! secret — the seed cannot introduce a leak the Layer wouldn't.

use lb_bus::{publish, Bus};
use lb_store::{capped_insert, new_ulid, Store};
use lb_telemetry::{TelemetryRecord, TABLE, TAIL_SUBJECT};

use super::error::TelemetrySvcError;

/// Write `record` into `ws`'s capped `telemetry` ring (per-source FIFO key, the production default)
/// and publish it onto the ws-walled tail subject so an attached `telemetry.tail` receives it live.
/// `cap` bounds the ring exactly as production. Returns the row's `seq` (its ULID id).
pub async fn telemetry_seed(
    store: &Store,
    bus: &Bus,
    ws: &str,
    cap: usize,
    record: &TelemetryRecord,
) -> Result<String, TelemetrySvcError> {
    let id = new_ulid();
    let body = record.to_value();
    // Per-source FIFO key (the production default: a chatty source can't evict a quiet one).
    let cap_key = if record.source.is_empty() {
        "_unknown"
    } else {
        &record.source
    };
    capped_insert(store, ws, TABLE, &id, cap_key, cap, &body).await?;

    // Mirror onto the tail subject (best-effort, like the Layer) so a live tail sees it. The stored
    // body carries the injected `seq`/`cap_key`; re-read shape is irrelevant — the tail forwards the
    // published JSON verbatim, so publish the same flat fields the snapshot exposes plus `seq`.
    let mut published = body;
    if let Some(obj) = published.as_object_mut() {
        obj.insert("seq".into(), serde_json::Value::String(id.clone()));
    }
    if let Ok(bytes) = serde_json::to_vec(&published) {
        let _ = publish(bus, ws, TAIL_SUBJECT, &bytes).await;
    }
    Ok(id)
}
