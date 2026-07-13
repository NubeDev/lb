//! `invite.list` — list invites in the caller's workspace (invites scope). Gated by
//! `mcp:invite.list:call`. Returns the full records (the Access console shows status, email, role,
//! team, minter, dates). The token hash is included (for revoke/resend addressing); the raw token
//! is never stored.

use lb_auth::Principal;
use lb_authz as raw;
use lb_authz::Invite;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::InviteError;

/// List all invites in workspace `ws`. Gated by `mcp:invite.list:call`.
pub async fn invite_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<Invite>, InviteError> {
    authorize_tool(principal, ws, "invite.list").map_err(|_| InviteError::Denied)?;
    Ok(raw::invite_list_raw(store, ws).await?)
}
