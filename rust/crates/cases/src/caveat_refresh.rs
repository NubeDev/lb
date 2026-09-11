//! **Refresh a case's caveat echo** — the live half of `Case.caveated`.
//!
//! `Case.caveated` is an ECHO of the primary insight's caveat state, written when the case opens.
//! The insight layer refreshes `Insight.caveats` on **every** raise, deliberately: a caveat is a
//! statement about the world *right now*, and `lb_insights::raise` documents why it must not be
//! merged — "a merge would make a caveat permanent, which is the failure mode that teaches
//! operators to ignore it".
//!
//! An echo written once at open and never revisited reintroduces exactly that, in both directions:
//!
//! - a case opened BEFORE the sensor it rests on broke never shows the soft-block, so a contractor
//!   is dispatched against a number nobody should trust — the one outcome the caveat exists to
//!   prevent;
//! - a case opened WHILE caveated keeps the badge after the sensor is fixed, which is the permanent
//!   caveat the insight layer went out of its way to avoid.
//!
//! So the echo is refreshed wherever the primary is re-raised, and this is the one place that
//! writes it. Idempotent by construction: unchanged ⇒ no write, no event.

use lb_store::Store;

use crate::case::Case;
use crate::case_event::EventKind;
use crate::error::CasesError;
use crate::event_append::append_event;
use crate::save::save;

/// Set `case.caveated` to `caveated`, if it is not already that.
///
/// Returns the updated case, or `None` when nothing changed — so a caller can tell a real
/// transition from a no-op without re-reading. A closed case is left alone: what the client was
/// told at resolution does not get rewritten afterwards.
pub async fn refresh_caveat(
    store: &Store,
    ws: &str,
    id: &str,
    caveated: bool,
    actor: &str,
    ts: u64,
) -> Result<Option<Case>, CasesError> {
    let Some(mut case) = crate::get::get(store, ws, id).await? else {
        return Err(CasesError::BadInput(format!("no such case: {id}")));
    };
    if case.closed || case.caveated == caveated {
        return Ok(None);
    }
    case.caveated = caveated;
    save(store, ws, &mut case, ts).await?;
    // Stated in the history, because "why did the contractor button switch off?" is a question an
    // operator will ask, and a flag that changes with no event is unanswerable.
    append_event(
        store,
        ws,
        id,
        EventKind::Caveat,
        actor,
        serde_json::json!({ "caveated": caveated }),
        ts,
    )
    .await?;
    Ok(Some(case))
}
