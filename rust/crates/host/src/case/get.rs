//! `case_get` — read one case by id over the capability gate (case-plane scope).

use lb_auth::Principal;
use lb_cases::Case;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::CaseSvcError;

/// Return the case at `(ws, id)`, or `None` if absent in this workspace. Gated by
/// `mcp:case.get:call` (workspace-first §7).
pub async fn case_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<Option<Case>, CaseSvcError> {
    authorize_tool(principal, ws, "case.get").map_err(|_| CaseSvcError::Denied)?;
    Ok(lb_cases::get(store, ws, id).await?)
}
