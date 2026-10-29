//! `save` — persist a whole case record (cases internal).
//!
//! Every mutating verb here reads the case, edits the struct, and writes it back; this is the one
//! place that last step lives, so `last_activity_ts` is bumped in ONE file rather than in each of
//! the eight verbs that would otherwise each have to remember. A verb that forgot would leave a
//! case that changed hands looking untouched in the queue's activity column.
//!
//! The SLA clock is kept here for the same reason ([`crate::sla_follow`]): it runs only while the
//! case is Open, and every move in or out of Open is a write through this file.

use lb_store::{write, Store};

use crate::case::{Case, TABLE};
use crate::error::CasesError;
use crate::event_append::append_event;
use crate::sla_follow::{follow, SLA_FOLLOW_ACTOR};

/// Write `case` back at `(ws, case.id)`, stamping `last_activity_ts = ts`, re-deriving `closed`
/// from the workflow, and stopping or starting the SLA clock as the write moves the case out of or
/// back into Open. All three are derived facts, so deriving them here means no caller can write a
/// record whose `closed` disagrees with its `workflow`, or whose clock runs while nobody waits.
pub(crate) async fn save(
    store: &Store,
    ws: &str,
    case: &mut Case,
    ts: u64,
) -> Result<(), CasesError> {
    case.last_activity_ts = ts;
    save_quiet(store, ws, case, ts).await
}

/// [`save`] without stamping `last_activity_ts`: for the platform keeping its own books (a
/// recomputed deadline), which is not activity on the case. Moving every open case's deadline at
/// once — an upgrade, a policy edit — must not make the whole queue look touched just now.
pub(crate) async fn save_quiet(
    store: &Store,
    ws: &str,
    case: &mut Case,
    ts: u64,
) -> Result<(), CasesError> {
    case.closed = case.workflow.is_terminal();
    let clock = follow(store, ws, case, ts).await?;
    let value = serde_json::to_value(&*case).map_err(CasesError::decode)?;
    write(store, ws, TABLE, &case.id, &value).await?;
    if let Some((kind, data)) = clock {
        append_event(store, ws, &case.id, kind, SLA_FOLLOW_ACTOR, data, ts).await?;
    }
    Ok(())
}
