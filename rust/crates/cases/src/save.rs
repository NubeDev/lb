//! `save` — persist a whole case record (cases internal).
//!
//! Every mutating verb here reads the case, edits the struct, and writes it back; this is the one
//! place that last step lives, so `last_activity_ts` is bumped in ONE file rather than in each of
//! the eight verbs that would otherwise each have to remember. A verb that forgot would leave a
//! case that changed hands looking untouched in the queue's activity column.

use lb_store::{write, Store};

use crate::case::{Case, TABLE};
use crate::error::CasesError;

/// Write `case` back at `(ws, case.id)`, stamping `last_activity_ts = ts` and re-deriving `closed`
/// from the workflow. Both are derived facts, so deriving them here means no caller can write a
/// record whose `closed` disagrees with its `workflow`.
pub(crate) async fn save(
    store: &Store,
    ws: &str,
    case: &mut Case,
    ts: u64,
) -> Result<(), CasesError> {
    case.last_activity_ts = ts;
    case.closed = case.workflow.is_terminal();
    let value = serde_json::to_value(&*case).map_err(CasesError::decode)?;
    write(store, ws, TABLE, &case.id, &value).await?;
    Ok(())
}
