//! `user.delete` — remove a user (workspace-scoped) and revoke their grants (admin-crud scope).
//! Gated by `mcp:user.manage:call`, workspace-first. **Workspace-scoped by default** (the resolved
//! open question): a principal may hold grants in several workspaces; this deletes the `(ws, user)`
//! record and revokes that workspace's grants only. Global removal is a separate, higher-gated
//! node-directory action — not this verb.
//!
//! Two effects in one logical op: tombstone the user record (the store has no row-delete; tombstone
//! is the sync-idempotent §6.8 choice) and call the **authz revoke seam** (`revoke_subject`) to
//! strip the user's grants — reusing slice 1's one revoke path, not a new one. Idempotent: deleting
//! an absent user is a success; re-deleting double-tombstones + double-revokes harmlessly.

use lb_auth::Principal;
use lb_authz::{revoke_subject, Subject};
use lb_mcp::authorize_tool;
use lb_store::{write, Store};

use super::error::UsersError;
use super::model::{TABLE, TOMBSTONE};

/// Delete `user` in workspace `ws` and revoke their grants there. Returns the number of caps
/// revoked (for the UI consequence note). Idempotent; never crosses the workspace wall.
pub async fn user_delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    user: &str,
) -> Result<usize, UsersError> {
    authorize_tool(principal, ws, "user.manage").map_err(|_| UsersError::Denied)?;
    // Tombstone the record (read-as-absent everywhere; survives sync without resurrection).
    let tombstone = serde_json::json!({ "kind": TOMBSTONE, "user": user, "active": false });
    write(store, ws, TABLE, user, &tombstone).await?;
    // Revoke the user's grants through the shared authz seam (slice 1). Live for Gate 3 on next
    // read; Gate-2 inherited caps drop on re-mint — but a deleted user also can't mint.
    let revoked = revoke_subject(store, ws, &Subject::User(user.to_string())).await?;
    Ok(revoked)
}
