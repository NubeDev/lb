//! The breach escalation — tell the person who owns the work that it went past its deadline
//! (case-plane scope §"Reactors", the sla-clock row).
//!
//! # It BYPASSES the ladder, for the reason `insight/assign_notify.rs` bypasses it
//!
//! `ladder_step` is per-`(sub, dedup_key)` anti-spam for **machine flapping** — escalate on
//! sustained noise, decay on quiet, one post per cooldown per key. A breach is none of those. It
//! happens **once per case, ever** (`breached_ts` is set-once), it is a contractual fact rather
//! than a firing, and routing it through the ladder would key it by the underlying detection's
//! `dedup_key` — so a flapping finding's own cooldown would swallow "this case is now in breach of
//! the SLA", the single message that must never be suppressed. The two signals share a key and mean
//! unrelated things.
//!
//! What the ladder was actually protecting against does not apply either: the volume bound here is
//! the breach itself, and the caller only reaches this file on the pass that actually wrote it.
//!
//! # Where it lands
//!
//! `assigned_to` is a **subject** (`user:priya`, `team:mechanical`), and a subject is also the name
//! of that owner's personal inbox channel — the `user:<sub>` convention `insight/notify.rs` already
//! uses for its dormant-subscription notes. So the escalation is one durable [`lb_inbox::Item`] in
//! the owner's own channel, with a stable id derived from the case, plus the ordinary best-effort
//! live bus echo. An unassigned case escalates to nobody: there is no owner to tell, and posting
//! into a guessed channel would be worse than silence.
//!
//! Best-effort throughout: the breach is already durable on the case by the time this runs (state
//! vs motion, README §3.3), so a channel hiccup must never fail the firing and un-breach the case.

use std::sync::Arc;

use lb_cases::Case;

use super::sla_clock::SLA_ACTOR;
use crate::boot::Node;

/// Post the breach notice into `case.assigned_to`'s own channel. No-op for an unassigned case.
pub(super) async fn escalate_to_assignee(node: &Arc<Node>, ws: &str, case: &Case, now: u64) {
    let Some(assignee) = case.assigned_to.as_deref() else {
        return;
    };

    let item = lb_inbox::Item::new(
        // Stable per case: a re-delivery upserts the same row rather than posting twice (the inbox
        // idempotency contract). Belt to the set-once brace in `lb_cases::mark_breached`.
        format!("case-breach:{}", case.id),
        assignee,
        SLA_ACTOR.to_string(),
        body(case),
        now,
    );
    if let Err(e) = lb_inbox::record(&node.store, ws, &item).await {
        tracing::warn!(ws, case_id = %case.id, error = %e, "breach notice not delivered; the breach itself is durable on the case");
        return;
    }
    // Create-on-first-post, so the owner's channel is listable. Best-effort: the durable item is
    // the truth.
    let _ =
        crate::channel_registry::register_on_post(&node.store, ws, assignee, SLA_ACTOR, now).await;
    if let Ok(payload) = serde_json::to_vec(&item) {
        let _ = lb_bus::publish(
            &node.bus,
            ws,
            &crate::channel::msg_key_for(assignee, &item.id),
            &payload,
        )
        .await;
    }
}

/// The notice. Names the case, the deadline it missed, and — the part that matters — **who the case
/// was waiting on at that instant**, so the first thing the owner reads is whether the ball was
/// ours. `waiting_on` is a closed lb enum, not workspace vocabulary, so naming its shapes here is
/// not a rule-10 special case.
fn body(case: &Case) -> String {
    let waiting = match case.breach_waiting_on {
        Some(who) => format!(" Waiting on: {who:?}."),
        None => String::new(),
    };
    let due = case
        .due_at
        .map(|d| d.to_string())
        .unwrap_or_else(|| "unknown".into());
    format!(
        "Case {} ({}) is in breach of its service level. Due at {due}.{waiting}",
        case.id, case.title
    )
}
