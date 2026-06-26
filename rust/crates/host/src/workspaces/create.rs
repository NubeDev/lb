//! `workspace_create` — register a workspace in the node directory so it appears in the switcher.
//!
//! Gated by `mcp:workspace.create:call` against the session's own workspace (from the token). Writes
//! the directory record in the reserved namespace; idempotent on the workspace id (re-creating upserts
//! the display name / ts). This does NOT provision the namespace's data — a workspace's namespace
//! springs into existence on first write to it; this only makes it *listable*.

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::{write, Store};

use super::error::WorkspacesError;
use super::model::{WorkspaceRecord, TABLE, WORKSPACES_NS};

/// Register workspace `ws` with display `name` in the directory, as `principal`. Returns the record.
pub async fn workspace_create(
    store: &Store,
    principal: &Principal,
    ws: &str,
    name: &str,
    ts: u64,
) -> Result<WorkspaceRecord, WorkspacesError> {
    authorize_tool(principal, principal.ws(), "workspace.create")
        .map_err(|_| WorkspacesError::Denied)?;
    let record = WorkspaceRecord::new(ws, name, ts);
    let value = serde_json::to_value(&record)
        .map_err(|e| lb_store::StoreError::Decode(e.to_string()))?;
    write(store, WORKSPACES_NS, TABLE, ws, &value).await?;
    Ok(record)
}
