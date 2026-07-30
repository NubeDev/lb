//! Per-recipient **delivered markers** — the at-least-once dedup ledger, at the outbox level.
//!
//! The outbox retries a failed effect *wholesale*. When an effect fans out to several recipients (an
//! email to one address today, a notification to N devices in `notify/delivered.rs`), a retry after a
//! partial failure would re-deliver the ones that already succeeded. A marker row keyed by
//! `(target, dedup_key, recipient)` — workspace-scoped like every key (§7) — is written after each
//! successful send and checked before the next attempt, so a retry only re-sends what actually failed.
//!
//! **The window this does not close.** The marker is written *after* the provider reports success, so a
//! crash between "the relay accepted the message" and "the marker is committed" still duplicates on
//! retry. That is irreducible without a two-phase commit the SMTP protocol does not offer, and email has
//! no collapse key. It is stated, not papered over as exactly-once. Two things narrow it: the marker
//! (this file) and a **stable `Message-ID` across retries** derived from the same dedup key, which lets
//! the receiving side collapse the duplicate (`lb_mail::MailMessage::message_id`).
//!
//! Generalized from `notify/delivered.rs` (which stays on its own `push_delivered` table for now — the
//! push markers in flight on a live node are keyed differently and migrating them buys nothing except a
//! one-time re-send). Same shape, one extra key component: the target string, so `email` and a future
//! `sms` for the same effect id cannot collide.

use lb_store::{read, write, Store, StoreError};

/// The store table outbox delivered-markers live in.
pub const OUTBOX_DELIVERED_TABLE: &str = "outbox_delivered";

fn marker_id(target: &str, dedup_key: &str, recipient: &str) -> String {
    format!("sent:{target}:{dedup_key}:{recipient}")
}

/// Has this `(target, effect, recipient)` triple already been delivered? Checked before every send.
pub async fn delivery_check(
    store: &Store,
    ws: &str,
    target: &str,
    dedup_key: &str,
    recipient: &str,
) -> Result<bool, StoreError> {
    Ok(read(
        store,
        ws,
        OUTBOX_DELIVERED_TABLE,
        &marker_id(target, dedup_key, recipient),
    )
    .await?
    .is_some())
}

/// Record a successful send for this `(target, effect, recipient)` triple. Idempotent upsert.
pub async fn delivery_mark(
    store: &Store,
    ws: &str,
    target: &str,
    dedup_key: &str,
    recipient: &str,
    ts: u64,
) -> Result<(), StoreError> {
    let value = serde_json::json!({
        "kind": "outbox_delivered",
        "target": target,
        "dedup_key": dedup_key,
        "recipient": recipient,
        "ts": ts,
    });
    write(
        store,
        ws,
        OUTBOX_DELIVERED_TABLE,
        &marker_id(target, dedup_key, recipient),
        &value,
    )
    .await
}
