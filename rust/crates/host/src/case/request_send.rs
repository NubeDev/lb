//! `case.request.send` — ask an external party something about a case, in one coherent step
//! (case-plane scope, wave 2).
//!
//! Gated on `mcp:case.request.send:call`, the AUTHOR (member) tier: asking a contractor to quote is
//! a triage act. It is deliberately its OWN cap rather than riding `case.workflow` — this is the
//! one verb on the plane that causes the platform to send mail to a company outside the workspace
//! under the customer's name, and "may move a case" and "may email the outside world" are grants an
//! admin will want to give different people.
//!
//! What one call does, in order:
//!   1. resolve the case and the party (both must be in THIS workspace; a party with no email is
//!      refused, because an ask nobody receives is not an ask);
//!   2. resolve the window — the party's own, else the policy's, else the default
//!      ([`super::request_window`]);
//!   3. mint a ≥256-bit token and keep only its SHA-256 hash — the raw token leaves this function
//!      exactly once, inside the effect payload, on its way into the mail;
//!   4. write the request row AND stage the link effect in **one transaction**
//!      (`lb_outbox::enqueue`) — there is no window in which an ask is durable but the link that
//!      makes it an ask was never scheduled (the invite precedent);
//!   5. schedule the 50 % / 80 % / breach ladder;
//!   6. set the case's `waiting_on` and append a `request_sent` event.
//!
//! Step 4 is why the record is built by [`lb_cases::new_request`] and persisted by the outbox's
//! transactional write rather than by the crate's own `request_open`.

use std::sync::Arc;

use lb_auth::Principal;
use lb_cases::{Ask, Brief, CaseRequest, EventKind, RequestInput, WaitingOn, CASE_REQUEST_TABLE};
use lb_mcp::authorize_tool;
use lb_store::new_ulid;

use super::error::CaseSvcError;
use super::request_link::link_effect;
use super::request_nudge_schedule::schedule_nudges;
use super::request_token::{generate_request_token, hash_request_token};
use super::request_window::{nudge_instants, window_hours_for, HOUR_MS};
use crate::boot::Node;

/// Raise an ask on `case_id` to `party_id`. Returns the durable request row.
///
/// The raw token is **not** returned: it belongs in the mail and nowhere else, and a verb that
/// handed it back would put a working link into every caller's response log.
// Every argument is a distinct fact about the ask and none of them group into a struct that
// would mean anything on its own — a bag named `Args` is the same arity with one more
// indirection between a caller and a mistake.
#[allow(clippy::too_many_arguments)]
pub async fn case_request_send(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    case_id: &str,
    party_id: &str,
    ask: Ask,
    brief: Option<Brief>,
    ts: u64,
) -> Result<CaseRequest, CaseSvcError> {
    authorize_tool(principal, ws, "case.request.send").map_err(|_| CaseSvcError::Denied)?;

    let store = &node.store;
    let Some(case) = lb_cases::get(store, ws, case_id).await? else {
        return Err(CaseSvcError::BadInput(format!("no such case: {case_id}")));
    };
    let Some(party) = lb_cases::party_get(store, ws, party_id).await? else {
        return Err(CaseSvcError::BadInput(format!("no such party: {party_id}")));
    };
    let Some(email) = party.contact.email.clone() else {
        return Err(CaseSvcError::BadInput(format!(
            "party {party_id} has no email address — there is nowhere to send the ask"
        )));
    };

    let window_h = window_hours_for(store, ws, &case, &party).await?;
    let expires_ts = ts + window_h as u64 * HOUR_MS;

    let token = generate_request_token(ws);
    let token_hash = hash_request_token(&token);
    let id = new_ulid();
    let effect = link_effect(&id, ws, &email, &token, None, ts);

    let request = lb_cases::new_request(
        RequestInput {
            id: id.clone(),
            case_id: case_id.to_string(),
            party_id: party_id.to_string(),
            ask,
            token_hash,
            expires_ts,
            // The same instant today: the deadline we state and the moment the door locks. Two
            // fields because they are two promises (see the record's doc) — a later policy may put
            // a grace period between them without changing this call site.
            respond_by: expires_ts,
            effect_id: Some(effect.id.clone()),
            brief,
        },
        ts,
    )?;

    // The ask and its mail, atomically. A crash after this point costs a nudge, never the link.
    let value = serde_json::to_value(&request)
        .map_err(|e| CaseSvcError::Store(format!("request encode: {e}")))?;
    lb_outbox::enqueue(store, ws, CASE_REQUEST_TABLE, &request.id, &value, &effect)
        .await
        .map_err(|e| CaseSvcError::Store(format!("stage the request link: {e}")))?;

    schedule_nudges(
        store,
        ws,
        &request.id,
        principal.sub(),
        nudge_instants(ts, window_h),
        ts,
    )
    .await?;

    // The ball is now with the party. `waiting_on` is what makes a breach attributable, so it is
    // recorded on the case rather than inferred from "is there an open request".
    let waiting_on = waiting_on_for(&party.kind);
    lb_cases::set_waiting_on(store, ws, case_id, waiting_on, ts).await?;
    lb_cases::append_event(
        store,
        ws,
        case_id,
        EventKind::RequestSent,
        principal.sub(),
        serde_json::json!({
            "request_id": request.id,
            "party_id": party_id,
            "ask": ask,
            "respond_by": request.respond_by,
            "window_h": window_h,
        }),
        ts,
    )
    .await?;

    Ok(request)
}

/// Who the case is now blocked on, from the party's KIND — a property of the data, not a name
/// (rule 10). A contractor or an FMS vendor is external work; an FM or the client is the money and
/// the authority side, which is what `waiting_on: client` means on this plane.
fn waiting_on_for(kind: &lb_cases::PartyKind) -> WaitingOn {
    match kind {
        lb_cases::PartyKind::Contractor | lb_cases::PartyKind::Fms => WaitingOn::Contractor,
        lb_cases::PartyKind::Fm | lb_cases::PartyKind::Client => WaitingOn::Client,
    }
}
