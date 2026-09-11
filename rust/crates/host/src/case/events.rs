//! `case_events` — the drawer's history list, paged, over the capability gate.
//!
//! Gates on `case.get` for the same reason [`super::members`] does: the history is the case's
//! detail, not a second privilege. Aliased `case.events → case.get` in `tool_gate.rs`.

use lb_auth::Principal;
use lb_cases::EventPage;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::CaseSvcError;

/// One page of `case_id`'s history, newest-first. Gated by `mcp:case.get:call`.
pub async fn case_events(
    store: &Store,
    principal: &Principal,
    ws: &str,
    case_id: &str,
    limit: usize,
    after: Option<u64>,
) -> Result<EventPage, CaseSvcError> {
    authorize_tool(principal, ws, "case.get").map_err(|_| CaseSvcError::Denied)?;
    if lb_cases::get(store, ws, case_id).await?.is_none() {
        return Err(CaseSvcError::BadInput(format!("no such case: {case_id}")));
    }
    Ok(lb_cases::events(store, ws, case_id, limit, after).await?)
}
