//! `sla_pause` — a snooze with an end date stops the SLA clock, and its end starts it again
//! (Insights review, 2026-10: "the SLA pauses on hold").
//!
//! **What pauses.** Only a snooze — a person saying "not now, until then, because…". A snooze
//! already needs a reason ([`crate::snooze`]); to pause it must also END: an `until` more than
//! [`MAX_SLA_PAUSE_MS`] away is open-ended ("indefinitely") and the clock keeps running, so a pause
//! cannot become a way to never breach. `waiting_on` — an email out to a contractor, a purchase
//! order — never pauses: who holds the ball is not a reason to stop time. Nor does a case with no
//! deadline, one already breached (a breach is history), or a closed one.
//!
//! **How a pause moves the deadline.** The deadlines stay a pure function, now of `(policy,
//! opened_ts, sla_paused_ms)` ([`crate::due_at`]). While a pause runs `sla_paused_at` is set, the
//! clock reads the case as it stood then ([`crate::sla_state`]) and no breach is recorded
//! ([`crate::mark_breached`]). When it ends — un-snooze, expiry or a severity puncture — the
//! business time between the two instants on the policy's calendar is added to `sla_paused_ms`, and
//! the next recompute moves both deadlines forward by exactly that. A pause over a weekend on a
//! weekday calendar adds nothing.
//!
//! One responsibility: start and end a case's SLA pause.

use lb_store::Store;

use crate::business_span::business_ms_between;
use crate::calendar::Calendar;
use crate::case::Case;
use crate::case_event::EventKind;
use crate::error::CasesError;
use crate::event_append::append_event;
use crate::save::save;

/// A snooze that ends further out than this is open-ended and leaves the clock running: a year.
pub const MAX_SLA_PAUSE_MS: u64 = 366 * 24 * 3_600_000;

/// Whether a snooze until `until`, set at `ts`, pauses `case`'s SLA.
pub fn snooze_pauses(case: &Case, until: u64, ts: u64) -> bool {
    !case.closed
        && case.due_at.is_some()
        && case.breached_ts.is_none()
        && until > ts
        && until - ts <= MAX_SLA_PAUSE_MS
}

/// Start the pause at `ts`. A no-op (no write, no event) when one is already running: changing a
/// snooze's end does not restart the clock it already stopped.
pub(crate) async fn pause(
    store: &Store,
    ws: &str,
    mut case: Case,
    actor: &str,
    ts: u64,
) -> Result<Case, CasesError> {
    if case.sla_paused_at.is_some() {
        return Ok(case);
    }
    case.sla_paused_at = Some(ts);
    save(store, ws, &mut case, ts).await?;
    let data = serde_json::json!({ "due_at": case.due_at, "policy_id": case.policy_id });
    append_event(store, ws, &case.id, EventKind::SlaPaused, actor, data, ts).await?;
    Ok(case)
}

/// End the running pause at `ts`, adding its business time to `sla_paused_ms`. A no-op when none is
/// running. The deadlines themselves move on the host's next recompute (the sla-clock), which is
/// the one path that writes them.
pub(crate) async fn resume(
    store: &Store,
    ws: &str,
    mut case: Case,
    actor: &str,
    ts: u64,
) -> Result<Case, CasesError> {
    let Some(from) = case.sla_paused_at.take() else {
        return Ok(case);
    };
    let calendar = policy_calendar(store, ws, case.policy_id.as_deref()).await?;
    let paused_ms = business_ms_between(&calendar, from, ts);
    case.sla_paused_ms = case.sla_paused_ms.saturating_add(paused_ms);
    save(store, ws, &mut case, ts).await?;
    let data = serde_json::json!({ "from": from, "paused_ms": paused_ms });
    append_event(store, ws, &case.id, EventKind::SlaResumed, actor, data, ts).await?;
    Ok(case)
}

/// The calendar the case's deadlines are measured on. A policy since deleted falls back to
/// wall-clock time: the pause was real time, and counting none of it would breach the case for
/// time it was told it had.
async fn policy_calendar(
    store: &Store,
    ws: &str,
    policy_id: Option<&str>,
) -> Result<Calendar, CasesError> {
    let Some(id) = policy_id else {
        return Ok(Calendar::Always);
    };
    Ok(crate::policy_list::policy_list(store, ws, true)
        .await?
        .into_iter()
        .find(|p| p.id == id)
        .map(|p| p.calendar)
        .unwrap_or(Calendar::Always))
}
