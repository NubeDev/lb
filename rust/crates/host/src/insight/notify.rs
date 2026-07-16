//! Shared notify plumbing for the raise path + the digest reactor (insight-notify + subscriptions
//! scopes). Loads the workspace's subscriptions, resolves the per-member kill-switch set, and
//! delivers a message into a sub's channel under its **stored principal, re-checked at fire time**
//! (the reminders pattern). On a fire-time deny the sub flips dormant + a note lands in the OWNER'S
//! inbox — never a silent stop. One responsibility per file (FILE-LAYOUT §3): the effectful shell
//! around the crate's pure `ladder_step` / `match_subs`.

use std::collections::HashSet;
use std::sync::Arc;

use lb_auth::Principal;
use lb_insights::{DormantReason, Subscription, SUB_TABLE};
use lb_store::{write, Store};

use crate::boot::Node;

/// Load every subscription in the workspace (the matcher + reactor both need the full set — the
/// 1000/ws cap keeps this bounded, subscriptions scope). Drains every scan page via
/// `lb_store::scan_all`; `scan` returns each `write`-based row as a `{ data: {...}, rev }` envelope,
/// so unwrap the inner `data` before decoding the `Subscription`. Best-effort: a store error returns
/// whatever was read so far (the reactor never fails a pass on a read hiccup).
pub async fn load_subs(store: &Store, ws: &str) -> Vec<Subscription> {
    let rows = lb_store::scan_all(store, ws, SUB_TABLE)
        .await
        .unwrap_or_default();
    let mut subs = Vec::with_capacity(rows.len());
    for row in rows {
        if let Some(sub) = decode_row::<Subscription>(row.data) {
            subs.push(sub);
        }
    }
    subs
}

/// Decode a scanned row, unwrapping the store's `data` envelope (`write`-based tables wrap the host
/// value under `data`; `scan` returns the whole record).
fn decode_row<T: serde::de::DeserializeOwned>(row: serde_json::Value) -> Option<T> {
    let inner = match row {
        serde_json::Value::Object(mut obj) => {
            obj.remove("data").unwrap_or(serde_json::Value::Object(obj))
        }
        other => other,
    };
    serde_json::from_value(inner).ok()
}

/// The set of sub-owner subjects whose per-member kill switch is OFF
/// (`Prefs.insight_notifications == Some(false)`). Deliveries for these owners are suppressed
/// (accounting continues — notify scope). Reads each distinct owner's prefs once.
pub async fn kill_off_owners(store: &Store, ws: &str, subs: &[Subscription]) -> HashSet<String> {
    let mut off = HashSet::new();
    let mut seen = HashSet::new();
    for sub in subs {
        if !seen.insert(sub.owner.clone()) {
            continue;
        }
        if let Ok(Some(prefs)) = lb_prefs::get_user_prefs(store, ws, &sub.owner).await {
            if prefs.insight_notifications == Some(false) {
                off.insert(sub.owner.clone());
            }
        }
    }
    off
}

/// Rebuild the sub's stored principal from its snapshot and re-authorize `bus:chan/{channel}:pub`
/// for a fire-time delivery. `Some(principal)` ⇒ authorized; `None` ⇒ denied (the caller flips the
/// sub dormant). The stored `principal` snapshot is the caps list captured at create; a revoke
/// since then is caught because the channel authorize gate re-runs against it.
fn refire_principal(sub: &Subscription, ws: &str) -> Option<Principal> {
    let caps: Vec<String> = serde_json::from_value(sub.principal.clone()).unwrap_or_default();
    let principal = Principal::routed(&sub.owner, ws, caps);
    // The same gate `channel::post` runs — a revoked/absent `bus:chan/{channel}:pub` denies here.
    match crate::channel::authorize_channel(&principal, ws, &sub.sink.channel, lb_caps::Action::Pub)
    {
        Ok(()) => Some(principal),
        Err(_) => None,
    }
}

/// Deliver `body` into the sub's channel under its stored principal (fire-time re-checked). On a
/// deny the sub is flipped dormant (`GrantRevoked`) and a note is posted to the OWNER'S inbox — the
/// sub never silently stops (subscriptions scope). `item_id` makes the post idempotent (a
/// re-delivery upserts the same channel item — the inbox idempotency contract).
pub async fn deliver_to_sub(
    node: &Arc<Node>,
    ws: &str,
    sub: &Subscription,
    item_id: &str,
    body: &str,
    now: u64,
) {
    let Some(principal) = refire_principal(sub, ws) else {
        mark_dormant(node, ws, sub, DormantReason::GrantRevoked, now).await;
        return;
    };
    let item = lb_inbox::Item::new(
        item_id.to_string(),
        sub.sink.channel.as_str(),
        principal.sub().to_string(),
        body.to_string(),
        now,
    );
    // Best-effort durable post + live echo (the record is the truth; a bus hiccup is non-fatal).
    let _ = lb_inbox::record(&node.store, ws, &item).await;
    let _ = crate::channel_registry::register_on_post(
        &node.store,
        ws,
        &sub.sink.channel,
        principal.sub(),
        now,
    )
    .await;
    if let Ok(payload) = serde_json::to_vec(&item) {
        let _ = lb_bus::publish(
            &node.bus,
            ws,
            &crate::channel::msg_key_for(&sub.sink.channel, &item.id),
            &payload,
        )
        .await;
    }
}

/// Flip a subscription dormant and drop one note into the owner's inbox (never a silent stop).
async fn mark_dormant(
    node: &Arc<Node>,
    ws: &str,
    sub: &Subscription,
    reason: DormantReason,
    now: u64,
) {
    let mut updated = sub.clone();
    updated.dormant_reason = Some(reason);
    updated.muted = true;
    if let Ok(value) = serde_json::to_value(&updated) {
        let _ = write(&node.store, ws, SUB_TABLE, &sub.id, &value).await;
    }
    // A system note to the OWNER'S personal inbox channel (`user:<sub>` convention), not the target
    // channel. Best-effort — the durable dormant flag above is the source of truth.
    let note = lb_inbox::Item::new(
        format!("insight-sub-dormant:{}", sub.id),
        sub.owner.as_str(),
        "system:insights".to_string(),
        format!(
            "Your insight subscription {} went dormant (channel delivery denied). Re-enable it once you regain access to '{}'.",
            sub.id, sub.sink.channel
        ),
        now,
    );
    let _ = lb_inbox::record(&node.store, ws, &note).await;
}
