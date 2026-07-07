//! `apply_intents` — the raise-path notify step (insight-notify-scope.md).
//!
//! The matcher (`match_subs`) turned a raise into per-sub `Intent`s; this runs each through the
//! pure `ladder_step`, persists the advanced `NotifyState`, and returns the deliveries to fire NOW
//! (L0-immediate posts + breakthroughs). Digest-window deliveries are NOT produced here — they come
//! from the reactor's `Tick` (`compute_due_digests`). Store-only (no channel I/O, no `&Node`): the
//! host posts the returned deliveries under each sub's stored principal (fire-time re-checked).

use std::collections::HashSet;

use lb_store::Store;

use crate::error::InsightsError;
use crate::intent::Intent;
use crate::ladder::{ladder_step, Delivery, LadderInput};
use crate::notify_store::{read_notify, write_notify};
use crate::policy::Policy;
use crate::subscription::Subscription;

/// Apply `intents` (from `match_subs`) for a raise at logical time `now`. Persists each ladder
/// state transition and returns the immediate deliveries. `acked` is the parent insight's
/// post-raise ack state (ack suppresses non-breakthrough deliveries — notify scope). `kill_off`
/// names owners whose per-member kill switch is off.
// SCOPE: docs/scope/insights/insight-notify-scope.md §"The state machine" (Intent path)
pub async fn apply_intents(
    store: &Store,
    ws: &str,
    intents: &[Intent],
    acked: bool,
    now: u64,
    policy: &Policy,
    subs: &[Subscription],
    kill_off: &HashSet<String>,
) -> Result<Vec<Delivery>, InsightsError> {
    let mut deliveries = Vec::new();
    for intent in intents {
        let sub = subs.iter().find(|s| s.id == intent.sub_id);
        let (throttle, muted, kill_on) = match sub {
            Some(s) => (
                s.throttle_override,
                s.muted || s.dormant_reason.is_some(),
                !kill_off.contains(&s.owner),
            ),
            None => continue, // sub vanished mid-raise — nothing to accumulate against
        };
        let prior = read_notify(store, ws, &intent.sub_id, &intent.dedup_key).await?;
        let (next, mut fired) = ladder_step(
            prior,
            LadderInput::Intent { intent, acked, now },
            policy,
            throttle,
            muted,
            kill_on,
        );
        write_notify(store, ws, &next).await?;
        deliveries.append(&mut fired);
    }
    Ok(deliveries)
}
