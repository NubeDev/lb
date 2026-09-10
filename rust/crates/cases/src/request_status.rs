//! The ask's **status transitions** — opened, withdrawn, expired (case-plane scope, wave 2).
//!
//! One file because they are one thing: where the *ask* is. What happened to the *email* is
//! [`crate::request_delivery`], and confusing the two is the bug this plane is built to avoid.
//!
//! All three are **idempotent and monotone**. A party who refreshes the page five times produces
//! one `opened_ts`; a withdraw of an already-withdrawn ask is a no-op; expiry is derived from the
//! clock rather than swept, so a request whose window passed while the node was down reads as
//! expired the moment anyone looks — no reactor, no backfill, nothing to miss (the same
//! derive-don't-sweep discipline the case snooze uses).
//!
//! **A terminal ask never moves again.** `replied` and `withdrawn` are final: opening a withdrawn
//! link does not resurrect it, and neither does the expiry helper.

use lb_store::Store;

use crate::case_request::{CaseRequest, RequestStatus};
use crate::error::CasesError;
use crate::request_save::request_save;

/// Record that the party opened the link, returning `true` when this was the FIRST open (the caller
/// appends a `request_opened` event only then — a refresh is not history).
///
/// A `replied`, `withdrawn` or already-`opened` request is untouched.
pub async fn request_mark_opened(
    store: &Store,
    ws: &str,
    request: &mut CaseRequest,
    ts: u64,
) -> Result<bool, CasesError> {
    if request.status != RequestStatus::Sent {
        return Ok(false);
    }
    request.status = RequestStatus::Opened;
    request.opened_ts = Some(ts);
    request_save(store, ws, request).await?;
    Ok(true)
}

/// Take the ask back. Returns `true` when this call was the one that withdrew it.
///
/// A **replied** request cannot be withdrawn: the answer already happened, and pretending otherwise
/// would erase the one record of what the party told us.
pub async fn request_withdraw(
    store: &Store,
    ws: &str,
    request: &mut CaseRequest,
    ts: u64,
) -> Result<bool, CasesError> {
    match request.status {
        RequestStatus::Replied => Err(CasesError::BadInput(
            "this request was already answered — a reply cannot be withdrawn".into(),
        )),
        RequestStatus::Withdrawn => Ok(false),
        _ => {
            request.status = RequestStatus::Withdrawn;
            // `expires_ts` is pulled back to now so the token dies with the ask rather than staying
            // live until the original window closed. Belt and braces: the gateway also refuses on
            // status, but a dead ask with a live token is a lock nobody turned.
            request.expires_ts = request.expires_ts.min(ts);
            request_save(store, ws, request).await?;
            Ok(true)
        }
    }
}

/// Whether `request` is past its window at logical `ts` — DERIVED, never swept.
pub fn is_expired(request: &CaseRequest, ts: u64) -> bool {
    request.status.is_live() && ts >= request.expires_ts
}

/// Persist the derived expiry when the window has closed on a still-live ask, so a reader that
/// looks after the fact sees `expired` rather than a `sent` row with a dead deadline. Returns
/// `true` when it wrote. A no-op for anything not live, and for a live ask still inside its window.
pub async fn request_expire_if_due(
    store: &Store,
    ws: &str,
    request: &mut CaseRequest,
    ts: u64,
) -> Result<bool, CasesError> {
    if !is_expired(request, ts) {
        return Ok(false);
    }
    request.status = RequestStatus::Expired;
    request_save(store, ws, request).await?;
    Ok(true)
}
