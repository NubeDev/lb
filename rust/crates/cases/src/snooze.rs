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
//! [`puncture_snooze`] is the third gesture and the one that matters most: a **severity escalation
//! punctures a snooze**. A case somebody parked as "watch it next quarter" that has since become
//! critical is not still parked — leaving it hidden is how a snooze turns into a way to lose a
//! fault. It lives here because clearing a snooze is this file's one responsibility, whoever asked.

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
    }
    save(store, ws, &mut case, ts).await?;

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
    Ok(case)
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
    Ok(Some(case))
}
