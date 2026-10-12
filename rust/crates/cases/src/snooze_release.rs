//! `release_to_action` — a case that comes out of a snooze goes back to "to action".
//!
//! A snooze is "come back to this later". When it ends — the time passes, somebody un-parks it, or
//! a severity escalation punctures it — the case is work waiting to be picked up again. A case left
//! in `actioned` would read as somebody's job in hand when nobody is on it, and self-clear (which
//! only judges untouched `to_action` cases) would never look at it. So the end of a snooze moves an
//! `actioned` case back to `to_action`, through [`crate::workflow`] so the move is an ordinary
//! `workflow` event in the history.
//!
//! Only `actioned` moves. `waiting_on_po` is blocked on somebody else and stays where it is;
//! `to_action` is already there; a resolved case is closed and a snooze ending does not reopen it.
//!
//! It is called by every door that ends a snooze — [`crate::snooze`] (un-park and expiry) and
//! [`crate::puncture_snooze`] — and only when the case WAS snoozed, so clearing a snooze that was
//! never set moves nothing.

use lb_store::Store;

use crate::case::{Case, Workflow};
use crate::error::CasesError;

/// Move `case` (just un-snoozed) from `actioned` back to `to_action` as `actor`; any other stage is
/// returned unchanged.
pub(crate) async fn release_to_action(
    store: &Store,
    ws: &str,
    case: Case,
    actor: &str,
    ts: u64,
) -> Result<Case, CasesError> {
    if case.workflow != Workflow::Actioned {
        return Ok(case);
    }
    crate::workflow::workflow(
        store,
        ws,
        &case.id,
        Workflow::ToAction,
        None,
        None,
        actor,
        ts,
    )
    .await
}
