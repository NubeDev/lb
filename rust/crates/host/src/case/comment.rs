//! `case_comment` — append a note to a case's history, over the triage capability.
//!
//! Gates on `mcp:case.workflow:call` (aliased `case.comment → case.workflow`) — see
//! [`super::assign`].
//!
//! `author` is **forced** to the principal's `sub` — the `ack.rs` host-stamp precedent. A caller
//! supplying `author: "user:someone-else"` is ignored, not refused: the field is simply not read
//! from the input, so there is no path by which a forged author reaches the store.

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::CaseSvcError;

/// Append `text` to case `id`'s history as `principal`, returning the assigned event `seq`.
pub async fn case_comment(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    text: &str,
    ts: u64,
) -> Result<u64, CaseSvcError> {
    authorize_tool(principal, ws, "case.workflow").map_err(|_| CaseSvcError::Denied)?;
    Ok(lb_cases::comment(store, ws, id, text, principal.sub(), ts).await?)
}
