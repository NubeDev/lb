//! `workspace_create` — register a workspace in the node directory so it appears in the switcher.
//!
//! A thin delegation into the atomic `provision` body (workspace-provision scope): admin = the
//! caller, default core skills. Gated by `mcp:workspace.create:call` against the session's own
//! workspace. Create therefore inherits provision's guarantees — the bootstrap applies as one atomic
//! batch and the directory row is written last, so the old best-effort `let _ =` path (which could
//! orphan a listable-but-memberless workspace) is gone. Idempotent: re-creating an existing active
//! workspace returns its current record and grants nothing; a purged tombstone is reported as the
//! would-be record without resurrecting (the pre-provision contract this verb keeps).

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::{read, Store};

use super::error::WorkspacesError;
use super::model::{WorkspaceRecord, TABLE, TOMBSTONE, WORKSPACES_NS};
use super::provision::provision_authorized;

/// Register workspace `ws` with display `name` in the directory, as `principal`, AND bootstrap the
/// creator as the first `workspace-admin` member — atomically. Returns the record.
pub async fn workspace_create(
    store: &Store,
    principal: &Principal,
    ws: &str,
    name: &str,
    ts: u64,
) -> Result<WorkspaceRecord, WorkspacesError> {
    authorize_tool(principal, principal.ws(), "workspace.create")
        .map_err(|_| WorkspacesError::Denied)?;
    // A purged (tombstoned) workspace must never resurrect via a re-create (admin-crud: tombstone
    // wins). Return the would-be record without overwriting — this verb's long-standing contract.
    if let Some(existing) = read(store, WORKSPACES_NS, TABLE, ws).await? {
        if existing.get("kind").and_then(|k| k.as_str()) == Some(TOMBSTONE) {
            return Ok(WorkspaceRecord::new(ws, name, ts));
        }
    }
    let report = provision_authorized(store, principal, ws, name, None, None, ts).await?;
    Ok(report.record)
}
