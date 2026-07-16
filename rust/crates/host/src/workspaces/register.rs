//! `workspace_register` — the **un-gated** directory-register seam (email-login scope). The
//! provisioning counterpart to [`workspace_create`](super::workspace_create): it writes ONLY the
//! directory record (no principal, no capability check, no first-member bootstrap), so a boot seed can
//! make a workspace *listable* before any login exists.
//!
//! Why this is needed: `login_workspaces` (the `/auth/login` roster) scans the workspace DIRECTORY and
//! keeps only the workspaces the person is an effective member of. Membership alone is not enough — the
//! workspace must also appear in the directory. `workspace_create` registers it, but it is
//! capability-gated and used to be reached only lazily on the first `/login`. A freshly seeded node has
//! the membership but not the directory record, so `/auth/login` returned "not a member of any
//! workspace" until a legacy `/login` ran. This seam lets the boot seed register the workspace up front.
//!
//! Idempotent, and it respects the purge tombstone (a purged workspace is never resurrected).

use lb_store::{read, write, Store, StoreError};

use super::model::{WorkspaceRecord, TABLE, TOMBSTONE, WORKSPACES_NS};

/// Register workspace `ws` (display `name`) in the node directory — the raw provisioning write, no
/// principal. Idempotent; a tombstoned (purged) workspace is left untouched (never resurrected).
pub async fn workspace_register(
    store: &Store,
    ws: &str,
    name: &str,
    ts: u64,
) -> Result<(), StoreError> {
    if let Some(existing) = read(store, WORKSPACES_NS, TABLE, ws).await? {
        if existing.get("kind").and_then(|k| k.as_str()) == Some(TOMBSTONE) {
            return Ok(()); // purged — do not resurrect
        }
    }
    let record = WorkspaceRecord::new(ws, name, ts);
    let value = serde_json::to_value(&record).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, WORKSPACES_NS, TABLE, ws, &value).await
}
