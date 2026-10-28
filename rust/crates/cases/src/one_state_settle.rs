//! `settle_one_state` — the migration for cases stored before "one state at a time" (Insights
//! review 2026-10): a case both ON HOLD and OUT BY EMAIL. Under the old rules a case emailed and then
//! held (or held and then emailed) kept both; now each ends the other ([`crate::snooze`],
//! [`crate::set_waiting_on`]), so a stored one is settled the same way: **the later of the two
//! stands**, read off the case's own history (its last `snoozed` and `request_sent` events). A later
//! hold ends the email-out; a later email ends the hold, SLA pause and all.
//!
//! Idempotent: a settled case is no longer both, so a second pass touches nothing.
//!
//! One responsibility: settle the stored cases that are in two states at once.

use lb_store::Store;

use crate::case::Case;
use crate::case_event::EventKind;
use crate::error::CasesError;

/// Who the history credits for a settle.
pub const SETTLE_ACTOR: &str = "system:one-state";

/// Both on hold (a running snooze) and out by email (lb sent it, the ball is still out).
fn in_two_states(case: &Case, now: u64) -> bool {
    !case.closed
        && case.snooze_until.is_some_and(|u| u > now)
        && case.emailed_by.is_some()
        && case.waiting_on.is_some()
}

/// Settle every open case in `ws` that is in two states at `now`. Returns how many were settled.
pub async fn settle_one_state(store: &Store, ws: &str, now: u64) -> Result<usize, CasesError> {
    let mut settled = 0;
    for case in crate::list_read::read_cases(store, ws, false, None).await? {
        if !in_two_states(&case, now) {
            continue;
        }
        let history = crate::events::events_all(store, ws, &case.id).await?;
        let last = |kind: EventKind| {
            history
                .iter()
                .filter(|e| e.kind == kind)
                .map(|e| e.ts)
                .max()
                .unwrap_or(0)
        };
        if last(EventKind::Snoozed) >= last(EventKind::RequestSent) {
            // The hold came last: the email-out is over. The request row stays (a reply still lands).
            let mut case = case;
            case.waiting_on = None;
            case.emailed_by = None;
            crate::save::save(store, ws, &mut case, now).await?;
        } else {
            crate::snooze::end_hold(store, ws, case, SETTLE_ACTOR, now).await?;
        }
        settled += 1;
    }
    Ok(settled)
}
