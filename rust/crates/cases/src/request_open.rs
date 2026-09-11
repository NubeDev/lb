//! `request_open` — mint the durable [`CaseRequest`] row (case-plane scope, wave 2).
//!
//! Everything unforgeable arrives already computed: the host mints the token and hands us only its
//! **hash**, resolves the window against the party and the policy, and injects the clock. This verb
//! therefore has exactly one job — write a well-formed row — and holds neither randomness nor
//! authorization nor a wall clock (testing §3), which is what makes the whole send path replayable
//! in a test with a fixed `ts`.
//!
//! The row starts `status: sent`, `delivery: queued`. Not `delivery: sent`: at this instant the
//! effect has been staged and nothing has been attempted, and the one thing this plane must never
//! do is claim a mail went out that did not (resolved decision 7).

use lb_store::Store;

use crate::case_request::{Ask, Brief, CaseRequest, Delivery, RequestStatus};
use crate::error::CasesError;
use crate::request_save::request_save;

/// The facts a caller must supply to raise an ask. A struct rather than nine positional arguments
/// because six of them are strings and two are timestamps — the shape where a positional list stops
/// being readable and starts being a bug waiting for a re-order.
#[derive(Debug, Clone)]
pub struct RequestInput {
    /// Host-assigned id (ULID), also the outbox idempotency handle.
    pub id: String,
    pub case_id: String,
    pub party_id: String,
    pub ask: Ask,
    /// SHA-256 (hex) of the raw token. The raw token never reaches this crate.
    pub token_hash: String,
    pub expires_ts: u64,
    pub respond_by: u64,
    /// The outbox effect the link email rides on.
    pub effect_id: Option<String>,
    /// The frozen snapshot the party is shown.
    pub brief: Option<Brief>,
}

/// Write a fresh request row at logical `ts` and return it.
pub async fn request_open(
    store: &Store,
    ws: &str,
    input: RequestInput,
    ts: u64,
) -> Result<CaseRequest, CasesError> {
    let request = new_request(input, ts)?;
    request_save(store, ws, &request).await?;
    Ok(request)
}

/// Build the row WITHOUT writing it — the same validation, the same shape, no store.
///
/// It exists because the send path stages the request row and its email effect in ONE transaction
/// (`lb_outbox::enqueue`, the invite precedent): there must be no window in which an ask is durable
/// but the link that makes it an ask was never scheduled. That transaction wants the record as a
/// value, so construction and persistence are separated here rather than the host hand-rolling a
/// second builder that could drift from this one.
pub fn new_request(input: RequestInput, ts: u64) -> Result<CaseRequest, CasesError> {
    if input.id.trim().is_empty() || input.id.contains(':') {
        return Err(CasesError::BadInput(
            "a request id must be a single non-empty record-id segment".into(),
        ));
    }
    if input.token_hash.trim().is_empty() {
        return Err(CasesError::BadInput(
            "a request needs a token hash — a link nobody can present is not an ask".into(),
        ));
    }
    if input.expires_ts <= ts {
        return Err(CasesError::BadInput(
            "a request's window must end in the future — an already-expired ask mails a dead link"
                .into(),
        ));
    }

    let request = CaseRequest {
        id: input.id,
        case_id: input.case_id,
        party_id: input.party_id,
        ask: input.ask,
        token_hash: input.token_hash,
        expires_ts: input.expires_ts,
        respond_by: input.respond_by,
        status: RequestStatus::Sent,
        delivery: Delivery::Queued,
        reply: None,
        sent_ts: ts,
        opened_ts: None,
        replied_ts: None,
        nudges_sent: 0,
        effect_id: input.effect_id,
        brief: input.brief,
    };
    Ok(request)
}
