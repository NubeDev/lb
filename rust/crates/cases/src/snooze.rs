//! `snooze` — park a case until a stated time, for a stated reason (case-plane scope).
//!
//! **A reason is required.** A snooze with no reason is an unexplained silence: three months later
//! nobody can tell "parked until the tenant moves out" from "somebody wanted it off their screen",
//! and the queue's credibility is exactly the sum of those answers.
//!
//! **Un-snooze is `until: now`** — the same verb, no second name. Passing an `until` at or before
//! the current logical time CLEARS the snooze (and needs no reason: ending a silence explains
//! itself). One gesture, one verb, and no state where `snooze_until` is in the past but the case
//! still reads as parked.
//!
//! **The end of a snooze is the end of the park.** Whichever way it ends — un-snooze, expiry (the
//! host's sweep calls the un-snooze gesture once `until` has passed) or a puncture — an `actioned`
//! case goes back to `to_action` ([`crate::snooze_release`]): parked work is waiting work again.
//!
//! [`puncture_snooze`] is the third gesture and the one that matters most: a **severity escalation
//! punctures a snooze**. A case somebody parked as "watch it next quarter" that has since become
//! critical is not still parked — leaving it hidden is how a snooze turns into a way to lose a
//! fault. It lives here because clearing a snooze is this file's one responsibility, whoever asked.
//!
//! **A snooze with an end date pauses the SLA** ([`crate::sla_pause`]); every door here that sets or
//! clears a snooze starts or ends that pause, so the two can never disagree.
//!
//! **A hold is a state, and a case is in one state at a time.** Snoozing a case that nobody has
//! started (`to_action`) moves it to `actioned` in the same call: held work is work in hand. And
//! [`end_hold`] is the fourth door — moving the case on (a stage change, a hand-over, an email)
//! ends a running hold, SLA pause and all, without sending it back to the queue: the new state
//! decides where it sits.

use lb_store::Store;

use crate::case::Case;
use crate::case_event::EventKind;
use crate::error::CasesError;
use crate::event_append::append_event;
use crate::save::save;

/// Snooze case `id` until `until` (a logical epoch-millis instant), for `reason`, as `actor`.
///
/// `until <= ts` is the UN-snooze gesture: the snooze fields clear and `reason` is not required.
/// Any future `until` requires a non-empty `reason`.
// SCOPE: docs/scope/insights/case-plane-scope.md §"Verbs" (case.snooze)
pub async fn snooze(
    store: &Store,
    ws: &str,
    id: &str,
    until: u64,
    reason: Option<&str>,
    actor: &str,
    ts: u64,
) -> Result<Case, CasesError> {
    let Some(mut case) = crate::get::get(store, ws, id).await? else {
        return Err(CasesError::BadInput(format!("no such case: {id}")));
    };

    let unsnoozing = until <= ts;
    let was_snoozed = case.snooze_until.is_some();
    if !unsnoozing && reason.map(str::trim).unwrap_or("").is_empty() {
        return Err(CasesError::BadInput(
            "case.snooze requires a `reason` — a parked case with no stated reason is an \
             unexplained silence in the queue; `until` at or before now un-snoozes and needs none"
                .into(),
        ));
    }

    if unsnoozing {
        case.snooze_until = None;
        case.snooze_reason = None;
        case.snoozed_by = None;
    } else {
        case.snooze_until = Some(until);
        case.snooze_reason = reason.map(str::to_string);
        case.snoozed_by = Some(actor.to_string());
        // One state at a time, the other way round from `set_waiting_on`: putting work on hold
        // ends its being out by email, so a held case never also reads as Emailed. Only an email
        // lb sent (`emailed_by`); a `waiting_on` set by hand is a fact about who has the ball and
        // stays. The request row itself is untouched — a party's reply still lands on the case.
        if case.emailed_by.is_some() {
            case.waiting_on = None;
            case.emailed_by = None;
        }
    }
    // Holding work nobody had started puts it in hand: `to_action` moves to `actioned` with the hold.
    let parked_from_open = !unsnoozing && case.workflow == crate::case::Workflow::ToAction;
    if parked_from_open {
        case.workflow = crate::case::Workflow::Actioned;
    }
    save(store, ws, &mut case, ts).await?;
    if parked_from_open {
        append_event(
            store,
            ws,
            id,
            EventKind::Workflow,
            actor,
            serde_json::json!({
                "from": crate::case::Workflow::ToAction,
                "to": crate::case::Workflow::Actioned,
                "resolution": null,
                "reason": null,
                "waiting_on": case.waiting_on,
            }),
            ts,
        )
        .await?;
    }

    append_event(
        store,
        ws,
        id,
        EventKind::Snoozed,
        actor,
        serde_json::json!({
            "until": case.snooze_until,
            "reason": case.snooze_reason,
            "cleared": unsnoozing,
        }),
        ts,
    )
    .await?;
    // The SLA pause follows the snooze ([`crate::sla_pause`]): one with an end date stops the
    // clock; ending it, or turning it open-ended, starts it again.
    let case = if !unsnoozing && crate::sla_pause::snooze_pauses(&case, until, ts) {
        crate::sla_pause::pause(store, ws, case, actor, ts).await?
    } else {
        crate::sla_pause::resume(store, ws, case, actor, ts).await?
    };
    if unsnoozing && was_snoozed {
        return crate::snooze_release::release_to_action(store, ws, case, actor, ts).await;
    }
    Ok(case)
}

/// End `case`'s running hold because it is moving to another state — a stage change, a hand-over,
/// an email. Clears the snooze fields, records a `snoozed` event marked `ended_by: "state_change"`
/// and ends the SLA pause (crediting the paused time), and does NOT send the case back to the queue:
/// the caller's new state decides where it sits. A case with no snooze is returned untouched (no
/// write, no event).
pub(crate) async fn end_hold(
    store: &Store,
    ws: &str,
    mut case: Case,
    actor: &str,
    ts: u64,
) -> Result<Case, CasesError> {
    if case.snooze_until.is_none() {
        return Ok(case);
    }
    case.snooze_until = None;
    case.snooze_reason = None;
    case.snoozed_by = None;
    save(store, ws, &mut case, ts).await?;
    append_event(
        store,
        ws,
        &case.id.clone(),
        EventKind::Snoozed,
        actor,
        serde_json::json!({ "until": null, "reason": null, "cleared": true, "ended_by": "state_change" }),
        ts,
    )
    .await?;
    crate::sla_pause::resume(store, ws, case, actor, ts).await
}

/// Clear a snooze because the case got WORSE — the severity escalation puncture.
///
/// Called by the host when a re-raise carries a higher severity than the case holds. Writes the new
/// severity and clears the snooze in one pass, appending a `snoozed` event marked `punctured` so
/// the history says why the case came back rather than just showing it un-parked.
///
/// A no-op (no write, no event) when the severity did not actually escalate — a re-raise at the
/// same severity is the ordinary case and must not un-park anything.
pub async fn puncture_snooze(
    store: &Store,
    ws: &str,
    id: &str,
    severity: &str,
    actor: &str,
    ts: u64,
) -> Result<Option<Case>, CasesError> {
    let Some(mut case) = crate::get::get(store, ws, id).await? else {
        return Err(CasesError::BadInput(format!("no such case: {id}")));
    };
    let escalated =
        crate::case::severity_rank(severity) > crate::case::severity_rank(&case.severity);
    if !escalated {
        return Ok(None);
    }
    let previous = std::mem::replace(&mut case.severity, severity.to_string());
    let was_snoozed = case.snooze_until.is_some();
    case.snooze_until = None;
    case.snooze_reason = None;
    case.snoozed_by = None;
    save(store, ws, &mut case, ts).await?;
    append_event(
        store,
        ws,
        id,
        EventKind::Snoozed,
        actor,
        serde_json::json!({
            "punctured": was_snoozed,
            "severity_from": previous,
            "severity_to": severity,
            "cleared": true,
        }),
        ts,
    )
    .await?;
    // A case that got worse is on the clock again.
    let case = crate::sla_pause::resume(store, ws, case, actor, ts).await?;
    if was_snoozed {
        return crate::snooze_release::release_to_action(store, ws, case, actor, ts)
            .await
            .map(Some);
    }
    Ok(Some(case))
}
