//! `request_set_delivery` / `request_bump_nudges` — what happened to the MAIL, and how many nudges
//! actually went (case-plane scope, wave 2).
//!
//! Separate from the ask's status transitions ([`crate::request_status`]) because they answer a
//! different question, and the drawer must be able to say "we asked them, and the email was
//! dropped" — a sentence that is unsayable if the two are one field.
//!
//! **`delivery` is monotone towards the truth.** It only ever moves `queued → {sent, logged,
//! failed}`; a later reconcile pass never walks it back to `queued`, because "we no longer know"
//! is not a thing that can become true about a mail we already attempted.

use lb_store::Store;

use crate::case_request::{CaseRequest, Delivery};
use crate::error::CasesError;
use crate::request_save::request_save;

/// Record the mail's outcome. Returns `true` when the value changed (so a reconcile pass that
/// finds nothing new does no write).
pub async fn request_set_delivery(
    store: &Store,
    ws: &str,
    request: &mut CaseRequest,
    delivery: Delivery,
) -> Result<bool, CasesError> {
    if request.delivery == delivery {
        return Ok(false);
    }
    if delivery == Delivery::Queued {
        // Never regress. See the module note.
        return Ok(false);
    }
    request.delivery = delivery;
    request_save(store, ws, request).await?;
    Ok(true)
}

/// Count one nudge that actually fired, returning the new total. The counter is the record of what
/// the party was subjected to, so it is bumped by the nudge verb AFTER the effect is staged, never
/// when the reminder is scheduled — three scheduled nudges on a request that was answered in ten
/// minutes must read as zero.
pub async fn request_bump_nudges(
    store: &Store,
    ws: &str,
    request: &mut CaseRequest,
) -> Result<u32, CasesError> {
    request.nudges_sent = request.nudges_sent.saturating_add(1);
    request_save(store, ws, request).await?;
    Ok(request.nudges_sent)
}
