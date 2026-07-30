//! `notify_assignment` — tell the people who asked that a finding became theirs
//! (`insight-assignee-notify-scope.md`).
//!
//! The triage plane shipped able to give someone work and tell them nothing. This is the arm that
//! closes it, and it makes one decision worth stating up front:
//!
//! **This deliberately BYPASSES the ladder.** `ladder_step` is per-`(sub, dedup_key)` anti-spam for
//! *machine flapping* — escalate on sustained noise, decay on quiet, one L0 post per cooldown per
//! key. An assignment is none of those: it is a one-shot act by a human, rate-limited by the human
//! doing it, and it is about *a person receiving work* rather than *a finding firing*. Routing it
//! through the ladder would key it by the insight's `dedup_key`, so **a flapping finding's own
//! cooldown would swallow "you have been assigned this"** — the one message that must not be
//! suppressed. The two signals share a key and mean unrelated things.
//!
//! Everything that makes a delivery *safe* still applies, because none of it lives in the ladder: the
//! sub's `muted` flag, the owner's per-member kill switch, `dormant_reason`, and the fire-time
//! re-check of `bus:chan/{channel}:pub` under the sub's stored principal (all inside
//! [`deliver_to_sub`]). Only the throttling is skipped, and only because the throttle models the
//! wrong thing.
//!
//! Two more rules, both about volume and both load-bearing:
//!   - **Bulk coalesces to ONE delivery per subscription.** "Assign these 12 to Priya" is one human
//!     gesture and must produce one notification naming 12, not twelve notifications. The verb holds
//!     every id, so the coalescing is free here — which is what makes bypassing the ladder affordable.
//!   - **Opt-in only.** A subscription receives an assignment notification ONLY if it filters on
//!     `assignee`. A sub without that axis is asking about *findings*, and must not start receiving a
//!     new event class it never requested — that is what keeps this strictly additive.

use std::sync::Arc;

use lb_auth::Principal;
use lb_insights::{assignee_matches, Insight};

use super::notify::{deliver_to_sub, kill_off_owners, load_subs};
use crate::boot::Node;

/// Notify the subscriptions that asked about `assignee` gaining `assigned` in workspace `ws`.
///
/// `assigned` is the set of insights this call actually assigned (successes only — a failed item
/// notifies nobody). Best-effort throughout: the assignment already landed durably, so a notify
/// hiccup must never fail the verb (state vs motion, README §3.3).
///
/// No-ops early and cheaply in the common cases: an un-assign (`assignee: None`), a self-assignment,
/// and a workspace where no subscription filters on an assignee.
// SCOPE: docs/scope/insights/insight-assignee-notify-scope.md §"Resolved decisions" (1–4)
pub async fn notify_assignment(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    assignee: Option<&str>,
    assigned: &[Insight],
    now: u64,
) {
    // Un-assignment notifies nobody (resolved decision 5): the feature is "you have been given
    // work", and losing it is not news worth a channel post.
    let Some(assignee) = assignee else {
        return;
    };
    if assigned.is_empty() {
        return;
    }
    // Self-assignment is silent (resolved decision 4). "I'll take this" is the most common triage
    // gesture, and telling someone about their own action is the noise that makes people mute a
    // channel. Assigning to a TEAM you are on still notifies — the assignee is the queue, not you,
    // and the rest of the crew needs to know.
    if principal.sub() == assignee {
        return;
    }

    let subs = load_subs(&node.store, ws).await;
    // Nothing to do at all unless some subscription opted in (see the gate in the loop).
    if !subs.iter().any(|s| s.filter.assignee.is_some()) {
        return;
    }

    let owner_subjects = super::assignee::owner_subjects_for(&node.store, ws, &subs).await;
    let kill_off = kill_off_owners(&node.store, ws, &subs).await;

    for sub in &subs {
        // ── THE OPT-IN GATE (resolved decision 3) ──────────────────────────────────────────────
        // A subscription without an `assignee` axis is asking about FINDINGS and must never receive
        // an assignment event. This single `else { continue }` is what makes the whole feature
        // additive: delete it and every existing subscription in every workspace silently becomes an
        // assignment feed the moment this ships. It is deliberately the ONLY place the opt-in is
        // decided — an incidental second gate (e.g. defaulting an absent filter to a value that
        // happens to match nothing) would make this line look removable when it is not.
        let Some(want) = sub.filter.assignee.as_deref() else {
            continue;
        };
        if sub.dormant_reason.is_some() || sub.muted {
            continue;
        }
        // The per-member kill switch silences the whole insight-notification system for an owner.
        // Unlike the ladder path there is no accounting to keep going here, so this is a plain skip.
        if kill_off.contains(&sub.owner) {
            continue;
        }
        if !assignee_matches(want, Some(assignee), &sub.owner, &owner_subjects) {
            continue;
        }
        // Count only the insights matching this sub's FULL filter — the severity floor, tags,
        // origin and dedup axes all still apply. The number in the message is what this subscriber
        // actually gained, not the number of ids the caller passed.
        let matched: Vec<&Insight> = assigned
            .iter()
            .filter(|i| non_assignee_axes_match(&sub.filter, i))
            .collect();
        if matched.is_empty() {
            continue;
        }
        // Idempotent per (sub, assignee, ts) — a retried bulk call at the same logical ts upserts
        // the same channel item rather than posting twice (the inbox idempotency contract).
        let item_id = format!("insight-assign:{}:{}:{}", sub.id, assignee, now);
        let body = assignment_body(assignee, &matched);
        deliver_to_sub(node, ws, sub, &item_id, &body, now).await;
    }
}

/// Every filter axis EXCEPT `assignee` (already decided by the caller). Mirrors the raise-time
/// matcher's AND semantics against a full [`Insight`] record rather than the matcher's lite view.
///
/// Tag matching reads the record's **echo** rather than the graph — the deliberate difference from
/// `insight.list {tags}`, which resolves through the graph because a *filter* must be correct even
/// while an echo lags. Here a stale echo can only mis-count one line of a notification, and reaching
/// the graph would mean a per-insight query on a bulk write path.
fn non_assignee_axes_match(filter: &lb_insights::SubFilter, insight: &Insight) -> bool {
    if let Some(origin_ref) = &filter.origin_ref {
        if origin_ref != &insight.origin.reference {
            return false;
        }
    }
    if let Some(dedup_key) = &filter.dedup_key {
        if dedup_key != &insight.dedup_key {
            return false;
        }
    }
    if let Some(floor) = filter.severity_min {
        if !insight.severity.at_least(floor) {
            return false;
        }
    }
    filter
        .tags
        .iter()
        .all(|(k, v)| insight.tags.get(k).is_some_and(|got| got == v))
}

/// The delivery body. Singular names the finding (that is the useful thing for one); plural names the
/// count, because listing twelve titles in a channel post is the spam this coalescing exists to stop.
///
/// Phrased as an EVENT ("were assigned"), never as a current count: the delivery is motion and the
/// roster is the truth, so by the time someone reads this the number may already be stale.
fn assignment_body(assignee: &str, matched: &[&Insight]) -> String {
    match matched {
        [one] => format!(
            "insight {} — “{}” was assigned to {} [view]",
            one.dedup_key, one.title, assignee
        ),
        many => format!(
            "{} insights were assigned to {} [view]",
            many.len(),
            assignee
        ),
    }
}
