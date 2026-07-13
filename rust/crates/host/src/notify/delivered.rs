//! Per-device **delivered markers** — the at-least-once dedup ledger for the push target
//! (push-target scope, Risks: "a retried effect must not double-buzz"). The outbox retries a
//! failed effect wholesale; without a per-device success record every retry would re-buzz the
//! devices that already succeeded. A marker row keyed by `(dedup_key, device_id)` — ws-scoped,
//! like every key (§7) — is written after each successful provider send and checked before the
//! next attempt, so a retry only re-sends the failures.
//!
//! The `dedup_key` is the effect's `idempotency_key` (the outbox's own dedup handle; falls back
//! to the effect id). `collapse_key` is NOT the marker key — it rides the payload to the provider
//! (WebPush `Topic` / FCM `collapse_key`) so the *provider* collapses stacked notifications;
//! distinct effects sharing a collapse key must each still be sent once.

use lb_store::{read, write, Store, StoreError};

/// The store table push delivered-markers live in.
pub const PUSH_DELIVERED_TABLE: &str = "push_delivered";

fn marker_id(dedup_key: &str, device_id: &str) -> String {
    format!("pushed:{dedup_key}:{device_id}")
}

/// Has this (effect, device) pair already been delivered? Checked before every provider send.
pub async fn delivered_check(
    store: &Store,
    ws: &str,
    dedup_key: &str,
    device_id: &str,
) -> Result<bool, StoreError> {
    Ok(read(
        store,
        ws,
        PUSH_DELIVERED_TABLE,
        &marker_id(dedup_key, device_id),
    )
    .await?
    .is_some())
}

/// Record a successful provider send for this (effect, device) pair. Idempotent upsert.
pub async fn delivered_mark(
    store: &Store,
    ws: &str,
    dedup_key: &str,
    device_id: &str,
    ts: u64,
) -> Result<(), StoreError> {
    let value = serde_json::json!({
        "kind": "push_delivered",
        "dedup_key": dedup_key,
        "device_id": device_id,
        "ts": ts,
    });
    write(
        store,
        ws,
        PUSH_DELIVERED_TABLE,
        &marker_id(dedup_key, device_id),
        &value,
    )
    .await
}
