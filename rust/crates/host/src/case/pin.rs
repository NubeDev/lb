//! `case_pin` — pin a case for yourself, or take your pin off.
//!
//! A pin is the caller's own bookmark ([`lb_cases::pin`]): the subject is the principal's, never read
//! from the input, so nobody can pin or unpin for someone else. Gated on the read grant
//! (`case.pin → case.list`): anyone who can see the queue may bookmark in it, a viewer included,
//! because a pin changes nothing anyone else sees.

use lb_auth::Principal;
use lb_cases::Case;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::CaseSvcError;

/// Pin (`pinned: true`) or unpin case `id` for `principal`.
pub async fn case_pin(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    pinned: bool,
) -> Result<Case, CaseSvcError> {
    authorize_tool(principal, ws, "case.list").map_err(|_| CaseSvcError::Denied)?;
    super::entity_filter::ensure_visible(store, principal, ws, id).await?;
    Ok(lb_cases::pin(store, ws, id, pinned, principal.sub()).await?)
}
