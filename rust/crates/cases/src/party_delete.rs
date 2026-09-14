//! `party_delete` — erase a party row, and REFUSE when that would orphan history
//! (case-plane scope §Wave 5).
//!
//! **Delete and disable are different answers to different questions**, and this file is only the
//! first one. [`crate::Party::active`] retires a party that has been used: it leaves the roster's
//! default read and takes no new work, while every request already sent to them still resolves
//! their name for the history and the nudge ladder. This verb is for the other case — a row created
//! by mistake, a duplicate, a test — where there is nothing to preserve.
//!
//! **So it refuses a party any `case_request` points at.** The alternative is a `case_request` whose
//! `party_id` resolves to nothing, which turns "who did we send this to?" — the question the whole
//! request plane exists to be able to answer — into a dangling id. The refusal names the count and
//! points at disabling, because the admin reaching for delete on a party with history almost always
//! wants retirement and does not know the word for it yet.
//!
//! Idempotent on a row that is not there: deleting nothing is not an error, and a caller retrying a
//! delete it already made should not be told it failed.

use lb_store::{delete, list as store_list, read, Store};

use crate::case_request::TABLE as REQUEST_TABLE;
use crate::error::CasesError;
use crate::party::TABLE;

/// Erase the party at `(ws, id)`.
///
/// Refuses with [`CasesError::BadInput`] when any `case_request` references it — disable the party
/// instead (`active: false`). Returns `Ok(false)` when there was no such row.
// SCOPE: docs/scope/insights/case-plane-scope.md §"Wave 5"
pub async fn party_delete(store: &Store, ws: &str, id: &str) -> Result<bool, CasesError> {
    if read(store, ws, TABLE, id).await?.is_none() {
        return Ok(false);
    }

    // `party_id` is a `data` field on the request row, so this is a single-field equality read
    // rather than a scan — the same door `case.request.list` uses.
    let referencing = store_list(store, ws, REQUEST_TABLE, "party_id", id)
        .await?
        .len();
    if referencing > 0 {
        return Err(CasesError::BadInput(format!(
            "party {id:?} cannot be deleted: {referencing} request(s) were sent to it, and erasing \
             the row would leave each of them pointing at nothing — set `active: false` to retire \
             it instead, which stops new asks and keeps the history readable"
        )));
    }

    delete(store, ws, TABLE, id).await?;
    Ok(true)
}
