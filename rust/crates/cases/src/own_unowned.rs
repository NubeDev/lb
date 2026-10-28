//! Work nobody owns is owned by whoever moves it on (Insights review 2026-10): a person who emails
//! a case out or puts it on hold, when it has no owner, becomes its owner — so an Emailed or held
//! case always has someone to chase the reply or pick it up when the hold ends. A system actor
//! (a rule, the sweep) never takes ownership; only a `user:`.

use lb_store::Store;

use crate::case::Case;
use crate::case_event::EventKind;
use crate::error::CasesError;
use crate::event_append::append_event;

/// Make `actor` the owner of `case` if it has none and `actor` is a person. True when it did; the
/// caller saves the case, then records the hand-over with [`record`].
pub(crate) fn claim(case: &mut Case, actor: &str) -> bool {
    if case.assigned_to.is_some() || !actor.starts_with("user:") {
        return false;
    }
    case.assigned_to = Some(actor.to_string());
    case.assigned_by = Some(actor.to_string());
    true
}

/// The `assigned` event for a [`claim`], once the case is saved.
pub(crate) async fn record(
    store: &Store,
    ws: &str,
    id: &str,
    actor: &str,
    ts: u64,
) -> Result<(), CasesError> {
    append_event(
        store,
        ws,
        id,
        EventKind::Assigned,
        actor,
        serde_json::json!({ "from": null, "to": actor }),
        ts,
    )
    .await?;
    Ok(())
}
