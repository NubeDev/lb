//! `case_members` — the drawer's citation list, paged, over the capability gate.
//!
//! **Gates on `case.get`, not a cap of its own.** The members ARE the case's detail: a reader who
//! may open the case may see which detections it covers, and a separate `mcp:case.members:call`
//! would exist in no role bundle — the shipped-but-unusable trap `tool_gate.rs` documents four
//! times. The alias `case.members → case.get` in that table is what makes the dispatcher agree.

use lb_auth::Principal;
use lb_cases::MemberPage;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::CaseSvcError;

/// One page of `case_id`'s members. Gated by `mcp:case.get:call`.
pub async fn case_members(
    store: &Store,
    principal: &Principal,
    ws: &str,
    case_id: &str,
    limit: usize,
    after: Option<&str>,
) -> Result<MemberPage, CaseSvcError> {
    authorize_tool(principal, ws, "case.get").map_err(|_| CaseSvcError::Denied)?;
    // Establish the case exists in THIS workspace first: without it a ws-B caller learns nothing,
    // but a caller in ws-A asking for a ws-B case id would get an empty page that reads as "a case
    // with no members" rather than "no such case".
    if lb_cases::get(store, ws, case_id).await?.is_none() {
        return Err(CaseSvcError::BadInput(format!("no such case: {case_id}")));
    }
    Ok(lb_cases::members(store, ws, case_id, limit, after).await?)
}
