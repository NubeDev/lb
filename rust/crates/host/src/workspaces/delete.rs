//! `workspace.delete` (soft archive) + `workspace.purge` (guarded hard delete) — the two-step
//! destructive workspace lifecycle (admin-crud scope). Soft-delete is the default the UI offers;
//! hard-delete is a separate verb with a **higher ceiling** so an admin never nukes a workspace with
//! one mis-click.
//!
//! - **`workspace_delete`** (soft): gated `mcp:workspace.delete:call`. Flips the directory record to
//!   `status=Archived` — hidden from the default `workspace.list` view, un-mintable, but reversible
//!   (`workspace.rename` un-archives; data retained). Idempotent; an absent workspace archives to a
//!   tombstone-free no-op success.
//! - **`workspace_purge`** (hard): gated `mcp:workspace.purge:call` — a *distinct* cap — **and** a
//!   typed `confirm` token that must equal the workspace id (neither a stray grant nor a stray click
//!   suffices: the resolved both-gates decision). Writes the directory **tombstone** so a stale
//!   synced edge cannot resurrect the workspace (§6.8 apply respects it). The namespace's data is
//!   left to GC (retain-until-purge; automatic GC is a later store follow-up).

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::{read, write, Store};

use super::error::WorkspacesError;
use super::model::{WorkspaceRecord, WorkspaceStatus, TABLE, TOMBSTONE, WORKSPACES_NS};

/// Soft-delete (archive) workspace `ws`. Reversible. Gated `mcp:workspace.delete:call`,
/// workspace-first. Idempotent (already-archived / absent → no-op success).
pub async fn workspace_delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<(), WorkspacesError> {
    authorize_tool(principal, principal.ws(), "workspace.delete")
        .map_err(|_| WorkspacesError::Denied)?;
    let Some(value) = read(store, WORKSPACES_NS, TABLE, ws).await? else {
        return Ok(()); // absent → nothing to archive.
    };
    if value.get("kind").and_then(|k| k.as_str()) == Some(TOMBSTONE) {
        return Ok(()); // already purged.
    }
    let mut record: WorkspaceRecord =
        serde_json::from_value(value).map_err(|e| lb_store::StoreError::Decode(e.to_string()))?;
    record.status = WorkspaceStatus::Archived;
    let value =
        serde_json::to_value(&record).map_err(|e| lb_store::StoreError::Decode(e.to_string()))?;
    write(store, WORKSPACES_NS, TABLE, ws, &value).await?;
    Ok(())
}

/// Hard-delete (purge) workspace `ws`. Irreversible. Requires the **distinct** purge cap AND a
/// `confirm` token equal to `ws` (type-the-name). Writes the directory tombstone (no resurrection).
/// Idempotent: re-purging re-writes the same tombstone.
pub async fn workspace_purge(
    store: &Store,
    principal: &Principal,
    ws: &str,
    confirm: &str,
) -> Result<(), WorkspacesError> {
    authorize_tool(principal, principal.ws(), "workspace.purge")
        .map_err(|_| WorkspacesError::Denied)?;
    // The typed confirm token must match the workspace id — a stray purge grant is not enough.
    if confirm != ws {
        return Err(WorkspacesError::Denied);
    }
    let tombstone = serde_json::json!({ "kind": TOMBSTONE, "ws": ws });
    write(store, WORKSPACES_NS, TABLE, ws, &tombstone).await?;
    Ok(())
}
