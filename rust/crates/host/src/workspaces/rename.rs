//! `workspace.rename` — update a workspace's display name, and un-archive it (admin-crud scope).
//! Gated by `mcp:workspace.delete:call` (the workspace-admin cap that also guards delete), against
//! the session's own workspace. Renaming sets `status=Active`, so it doubles as the **un-archive**
//! (reverse of the soft-delete) the scope's example flow uses.
//!
//! Idempotent: renaming to the same name is a no-op-equivalent upsert. A purged (tombstoned)
//! workspace is **not** renameable back to life — the tombstone wins (no resurrection).

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::{read, write, Store};

use super::error::WorkspacesError;
use super::model::{WorkspaceRecord, WorkspaceStatus, TABLE, TOMBSTONE, WORKSPACES_NS};

/// Rename workspace `ws` to `name` and mark it `Active`. The caller's own workspace must match `ws`
/// at the gate (workspace-first); a purged workspace is refused (returns `Ok` no-op is wrong — a
/// resurrection; we leave the tombstone untouched and report nothing changed).
pub async fn workspace_rename(
    store: &Store,
    principal: &Principal,
    ws: &str,
    name: &str,
    ts: u64,
) -> Result<(), WorkspacesError> {
    authorize_tool(principal, principal.ws(), "workspace.delete")
        .map_err(|_| WorkspacesError::Denied)?;
    if let Some(existing) = read(store, WORKSPACES_NS, TABLE, ws).await? {
        if existing.get("kind").and_then(|k| k.as_str()) == Some(TOMBSTONE) {
            return Ok(()); // purged — the tombstone is final, never resurrect.
        }
    }
    let mut record = WorkspaceRecord::new(ws, name, ts);
    record.status = WorkspaceStatus::Active;
    let value =
        serde_json::to_value(&record).map_err(|e| lb_store::StoreError::Decode(e.to_string()))?;
    write(store, WORKSPACES_NS, TABLE, ws, &value).await?;
    Ok(())
}
