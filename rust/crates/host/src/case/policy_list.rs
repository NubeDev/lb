//! `policy.sla.list` — every SLA service policy in the workspace (case-plane scope).
//!
//! **Admin**, alongside `policy.sla.set`. The list is not merely a read of the settings table: the
//! rows come back **most specific first**, which is the order the resolver applies them in, so an
//! admin can answer "why did this case get that deadline?" by reading down the page rather than by
//! reading the code.
//!
//! Admin rather than viewer because a policy row states the commercial contract — response and
//! resolution hours per site — and that is not something every member of a workspace is entitled to
//! read. A viewer sees the *consequence* (their case's `due_at`), never the terms.

use lb_auth::Principal;
use lb_cases::ServicePolicy;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::CaseSvcError;

/// Read every policy row in `ws`, ordered most-specific-first (3 pinned match axes → 2 → 1 → the
/// workspace default), ties broken by `id` ascending.
pub async fn case_policy_sla_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
    include_disabled: bool,
) -> Result<Vec<ServicePolicy>, CaseSvcError> {
    authorize_tool(principal, ws, "policy.sla.list").map_err(|_| CaseSvcError::Denied)?;
    Ok(lb_cases::policy_list(store, ws, include_disabled).await?)
}
