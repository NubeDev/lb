//! `set_waiting_on` — record WHO holds the ball, without moving the workflow (case-plane scope).
//!
//! Separate from [`crate::workflow`] because they are orthogonal axes, and because the caller that
//! needs this one — sending an ask to a contractor — is not making a workflow transition and must
//! not append a `workflow` event saying it went from `actioned` to `actioned`. The ask's own
//! `request_sent` event is the history of what happened; a phantom transition beside it would be
//! noise in the one list a client reads back six months later.
//!
//! `waiting_on` is what makes a breach attributable (`breach_waiting_on` is a snapshot of it), so
//! it is set on the record rather than derived from "is there an open request" — the request may be
//! withdrawn, replied and re-raised while the ball stays firmly with the contractor.

use lb_store::Store;

use crate::case::{Case, WaitingOn};
use crate::error::CasesError;
use crate::save::save;

/// Set case `id`'s `waiting_on` in workspace `ws` at logical ts `ts`, returning the saved case.
/// Bumps `last_activity_ts` like every write (that is [`crate::save`]'s job, in one place).
pub async fn set_waiting_on(
    store: &Store,
    ws: &str,
    id: &str,
    waiting_on: WaitingOn,
    ts: u64,
) -> Result<Case, CasesError> {
    let Some(mut case) = crate::get::get(store, ws, id).await? else {
        return Err(CasesError::BadInput(format!("no such case: {id}")));
    };
    case.waiting_on = Some(waiting_on);
    save(store, ws, &mut case, ts).await?;
    Ok(case)
}

/// Set the case's `cost_to_fix` — the accepted quote (`case-plane scope`, the money table:
/// "`cost_to_fix` ← accepted quote / PO amount from the request reply").
///
/// Here rather than in the reply verb because it is a write to the CASE, and the case's fields are
/// this crate's business; the reply verb's job is the request row. A non-finite or negative amount
/// is refused by `validate_reply` before this is ever reached.
pub async fn set_cost_to_fix(
    store: &Store,
    ws: &str,
    id: &str,
    amount: f64,
    ts: u64,
) -> Result<Case, CasesError> {
    let Some(mut case) = crate::get::get(store, ws, id).await? else {
        return Err(CasesError::BadInput(format!("no such case: {id}")));
    };
    case.cost_to_fix = Some(amount);
    save(store, ws, &mut case, ts).await?;
    Ok(case)
}
