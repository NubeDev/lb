//! `workspace_list` — every workspace in the node directory, for the switcher.
//!
//! Gated by `mcp:workspace.list:call` against the session's own workspace (from the token) — a
//! principal must hold the verb in its workspace to read the directory. The directory is node-level
//! config (which workspaces exist), so the list is the same for any authorized caller; login then
//! still gates which of them a principal can actually obtain a token for. Reads the reserved
//! namespace via the constant `kind` discriminant, sorted by `ts`.

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::{list as store_list, Store};

use super::error::WorkspacesError;
use super::model::{WorkspaceRecord, KIND, TABLE, WORKSPACES_NS};

/// Return every workspace in the node directory for `principal`, oldest→newest by `ts`.
pub async fn workspace_list(
    store: &Store,
    principal: &Principal,
) -> Result<Vec<WorkspaceRecord>, WorkspacesError> {
    authorize_tool(principal, principal.ws(), "workspace.list")
        .map_err(|_| WorkspacesError::Denied)?;
    let rows = store_list(store, WORKSPACES_NS, TABLE, "kind", KIND).await?;
    let mut records: Vec<WorkspaceRecord> = rows
        .into_iter()
        .map(|v| {
            serde_json::from_value(v).map_err(|e| lb_store::StoreError::Decode(e.to_string()).into())
        })
        .collect::<Result<_, WorkspacesError>>()?;
    records.sort_by_key(|r| r.ts);
    Ok(records)
}
