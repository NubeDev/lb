//! `request_reply` — record the party's answer, exactly once (case-plane scope, wave 2).
//!
//! **Idempotent by construction.** A contractor on a flaky phone signal taps *Send* twice; the
//! browser retries a POST it never saw acknowledged. The second call must not append a second
//! `case_event`, must not move the case a second time, and must not double the quote. So this verb
//! returns [`ReplyOutcome::Recorded`] exactly once per request and [`ReplyOutcome::Replayed`]
//! afterwards, and the host appends the event and applies the transition **only** on `Recorded`.
//!
//! That is deliberately *not* "compare the two bodies and allow a correction": a request is one ask
//! with one answer, and letting a token rewrite an answer already acted on (a quote already turned
//! into a PO) is a hole with no upside. A party that got it wrong tells us in a new ask.
//!
//! The window is closed here too, not only at the gateway: a reply arriving after `expires_ts`, or
//! on a withdrawn ask, is refused. The route and the verb both check, because the verb is also
//! reachable from a reminder-driven internal call and "the gateway checked it" is not a property
//! this crate can assert.

use lb_store::Store;

use crate::case_request::{validate_reply, CaseRequest, Reply, RequestStatus};
use crate::error::CasesError;
use crate::request_save::request_save;
use crate::request_status::is_expired;

/// Whether this call was the one that recorded the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplyOutcome {
    /// First time: the caller should append the `reply` event and apply the transition.
    Recorded,
    /// A replay of an answer already stored: the caller must do nothing else.
    Replayed,
}

/// Store `reply` against `request` at logical `ts`.
///
/// Refuses a reply on a withdrawn ask or past the window; validates the reply's own shape (a quote
/// carries an amount, text and attachments are bounded) BEFORE writing anything.
pub async fn request_reply(
    store: &Store,
    ws: &str,
    request: &mut CaseRequest,
    reply: Reply,
    ts: u64,
) -> Result<ReplyOutcome, CasesError> {
    if request.status == RequestStatus::Replied {
        return Ok(ReplyOutcome::Replayed);
    }
    if request.status == RequestStatus::Withdrawn {
        return Err(CasesError::BadInput(
            "this request was withdrawn — it can no longer be answered".into(),
        ));
    }
    if request.status == RequestStatus::Expired || is_expired(request, ts) {
        return Err(CasesError::BadInput(
            "this request's window has closed".into(),
        ));
    }
    validate_reply(&reply)?;

    request.reply = Some(reply);
    request.status = RequestStatus::Replied;
    request.replied_ts = Some(ts);
    // A party that replies without ever having "opened" (a mail client prefetch, a POST straight
    // from a saved form) still opened it in every sense that matters to the history.
    if request.opened_ts.is_none() {
        request.opened_ts = Some(ts);
    }
    request_save(store, ws, request).await?;
    Ok(ReplyOutcome::Recorded)
}
