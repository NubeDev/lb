//! `case.request.view` — what the contractor's page is allowed to know (case-plane scope, wave 2).
//!
//! **Token-only.** Its capability exists in no role bundle, and the verb additionally requires the
//! principal to be scoped to exactly this request ([`super::request_scope`]). Both halves matter:
//! the cap keeps every logged-in caller out, the scope keeps one token out of another token's ask.
//!
//! **The payload is a struct, not the record.** What the page cannot show is enforced by the type:
//! there is no case id, no insight, no member list, no history, no other party, no money beyond the
//! currency a quote is expected in. A contractor asked to price one job learns about that one job.
//! Returning `CaseRequest` + `Case` and letting the UI pick would make every future field on either
//! record a disclosure decision nobody is making on purpose.
//!
//! **The evidence rows travel with it.** The token principal holds two caps and is `Denied` on
//! every query-plane verb, so the page cannot fetch a chart — the rows come from the `brief`
//! snapshot frozen at send time. That is also the honest semantics: the party sees what they were
//! shown, and it does not change under them while they decide.

use lb_auth::Principal;
use lb_cases::{Ask, BriefEvidence, CaseRequest, EventKind, Reply};
use lb_mcp::authorize_tool;
use lb_store::Store;
use serde::Serialize;

use super::error::CaseSvcError;
use super::request_scope::request_id_in_scope;

/// Everything the tokenised page renders, and nothing else.
#[derive(Debug, Clone, Serialize)]
pub struct RequestView {
    pub request_id: String,
    pub ask: Ask,
    /// What we are asking, in the sender's words (from the frozen brief).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask_text: Option<String>,
    pub respond_by: u64,
    pub expires_ts: u64,
    pub party_name: String,
    /// The case's one-line title. The only thing from the case a party sees by name.
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset: Option<String>,
    pub severity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<BriefEvidence>,
    /// Present once answered, so the page can render the replied state instead of an empty form.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply: Option<Reply>,
}

/// Render the view for `request` as the token principal.
///
/// Records the first open on the row and appends a `request_opened` event attributed to the party —
/// a refresh does neither, so "they read it" stays a single fact in the history.
pub async fn case_request_view(
    store: &Store,
    principal: &Principal,
    ws: &str,
    request_id: &str,
    ts: u64,
) -> Result<RequestView, CaseSvcError> {
    authorize_tool(principal, ws, "case.request.view").map_err(|_| CaseSvcError::Denied)?;
    request_id_in_scope(principal, request_id)?;

    let Some(mut request) = lb_cases::request_get(store, ws, request_id).await? else {
        // Unreachable through the gateway (the token resolved the row), and still opaque: a scoped
        // principal whose row vanished learns nothing it did not already know.
        return Err(CaseSvcError::Denied);
    };

    if lb_cases::request_mark_opened(store, ws, &mut request, ts).await? {
        lb_cases::append_event(
            store,
            ws,
            &request.case_id,
            EventKind::RequestOpened,
            principal.sub(),
            serde_json::json!({ "request_id": request.id }),
            ts,
        )
        .await?;
    }

    view_of(store, ws, &request).await
}

/// Assemble the payload from the request, its party and its case.
async fn view_of(
    store: &Store,
    ws: &str,
    request: &CaseRequest,
) -> Result<RequestView, CaseSvcError> {
    let party = lb_cases::party_get(store, ws, &request.party_id).await?;
    let case = lb_cases::get(store, ws, &request.case_id).await?;
    let brief = request.brief.clone().unwrap_or_default();
    Ok(RequestView {
        request_id: request.id.clone(),
        ask: request.ask,
        ask_text: brief.ask_text,
        respond_by: request.respond_by,
        expires_ts: request.expires_ts,
        // An absent party or case is not an error here: the ask is the record that matters and the
        // page must still render. Empty strings, never a placeholder that could be read as data.
        party_name: party.map(|p| p.name).unwrap_or_default(),
        title: case.as_ref().map(|c| c.title.clone()).unwrap_or_default(),
        site: case.as_ref().and_then(|c| c.site.clone()),
        asset: brief.asset,
        severity: case.map(|c| c.severity).unwrap_or_default(),
        currency: brief.currency,
        evidence: brief.evidence,
        reply: request.reply.clone(),
    })
}
