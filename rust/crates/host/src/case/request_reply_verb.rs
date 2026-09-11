//! `case.request.reply` — the contractor answers (case-plane scope, wave 2).
//!
//! **Token-only and record-scoped**, exactly like `case.request.view`: the cap is in no bundle, and
//! the verb refuses a principal not scoped to this request.
//!
//! What one reply does:
//!   1. record the answer on the request — **idempotently**: a replayed POST returns the same
//!      result and writes nothing more (`lb_cases::request_reply` decides which call was the real
//!      one, so the "exactly one event" property lives in one place, not in every caller);
//!   2. apply the documented `workflow` / `waiting_on` transition
//!      ([`lb_cases::transition_for`]);
//!   3. write `cost_to_fix` when the reply is a quote — the money the whole payback figure divides
//!      by;
//!   4. cancel the nudge ladder (they answered; chasing them now is the platform being rude);
//!   5. append a `reply` event **attributed to `party:{id}`**.
//!
//! Step 5's attribution is the reason `case_event.actor` is a free subject string rather than a user
//! id. Six months later the history has to say *the contractor* quoted $1,180, not that some
//! service account did.
//!
//! Attachments are asset ids the upload route already wrote (a gateway route, not a third cap — the
//! token principal's surface stays these two verbs). They are validated as single record-id
//! segments before anything is stored: a `.` in an asset id breaks `store:asset/{id}:write` and the
//! file becomes unreadable after a successful-looking upload.

use lb_auth::Principal;
use lb_cases::{EventKind, Reply, ReplyKind, ReplyOutcome};
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::CaseSvcError;
use super::request_nudge_schedule::cancel_nudges;
use super::request_scope::request_id_in_scope;

/// The outcome of a reply, as the route reports it back to the page.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ReplyReceipt {
    pub request_id: String,
    /// `true` only for the call that actually recorded the answer. A replay says `false` and the
    /// page renders the same "thank you" — the party must not be able to tell, and must not be
    /// shown an error for a network retry that succeeded.
    pub recorded: bool,
}

/// Record `reply` against `request_id` as the token principal, at logical `ts`.
pub async fn case_request_reply(
    store: &Store,
    principal: &Principal,
    ws: &str,
    request_id: &str,
    reply: Reply,
    ts: u64,
) -> Result<ReplyReceipt, CaseSvcError> {
    authorize_tool(principal, ws, "case.request.reply").map_err(|_| CaseSvcError::Denied)?;
    request_id_in_scope(principal, request_id)?;

    let Some(mut request) = lb_cases::request_get(store, ws, request_id).await? else {
        return Err(CaseSvcError::Denied);
    };

    let kind = reply.kind;
    let amount = reply.amount;
    let outcome = lb_cases::request_reply(store, ws, &mut request, reply.clone(), ts).await?;
    if outcome == ReplyOutcome::Replayed {
        return Ok(ReplyReceipt {
            request_id: request.id,
            recorded: false,
        });
    }

    let (workflow, waiting_on) = lb_cases::transition_for(kind);
    lb_cases::workflow(
        store,
        ws,
        &request.case_id,
        workflow,
        None,
        Some(waiting_on),
        principal.sub(),
        ts,
    )
    .await?;

    if kind == ReplyKind::Quote {
        if let Some(amount) = amount {
            lb_cases::set_cost_to_fix(store, ws, &request.case_id, amount, ts).await?;
        }
    }

    cancel_nudges(store, ws, &request.id).await;

    lb_cases::append_event(
        store,
        ws,
        &request.case_id,
        EventKind::Reply,
        // `party:{id}` — the principal's own sub. Host-stamped from the token, never caller-supplied.
        principal.sub(),
        serde_json::json!({
            "request_id": request.id,
            "kind": kind,
            "amount": amount,
            "currency": reply.currency,
            "eta_ts": reply.eta_ts,
            "text": reply.text,
            "attachments": reply.attachments,
        }),
        ts,
    )
    .await?;

    Ok(ReplyReceipt {
        request_id: request.id,
        recorded: true,
    })
}
