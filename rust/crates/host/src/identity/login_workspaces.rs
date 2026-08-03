//! `login_workspaces` — the **login-path** membership enumeration (email-login scope). The un-gated
//! sibling of `identity.workspaces` (which is admin-gated): `/auth/login` calls this AFTER the
//! credential verifies but BEFORE any principal exists, to build the 0/1/N branch. It returns the
//! workspaces `sub` is an EFFECTIVE member of — i.e. the ones with a live `membership` row.
//!
//! There is no longer a per-workspace *disable* filter: the legacy `user` record (and its `active`
//! flag) was removed in the pre-production legacy sweep, so the only way to stop a person entering a
//! workspace is `membership.remove` (which also revokes their grants and marks the live token). See
//! `docs/scope/auth-caps/email-login-scope.md` — "Behaviour deliberately lost".
//!
//! Same bounded hub scan as `identity.workspaces` (a hub hosts few workspaces; this runs once at
//! login), same `{ws, name}` rows the picker renders. No authorization — pre-principal, exactly like
//! the credential verify.

use lb_store::Store;

use super::error::IdentityError;
use super::model::IdentityWorkspace;
use super::workspaces::is_effective_member;
use crate::workspaces::{
    WorkspaceRecord, WorkspaceStatus, KIND as WS_KIND, TABLE as WS_TABLE, WORKSPACES_NS,
};

/// The workspaces `sub` may log into: the active workspaces it is a member of. Sorted by `ws`.
/// Bounded by the directory size.
pub async fn login_workspaces(
    store: &Store,
    sub: &str,
) -> Result<Vec<IdentityWorkspace>, IdentityError> {
    let rows = lb_store::list(store, WORKSPACES_NS, WS_TABLE, "kind", WS_KIND).await?;
    let mut out = Vec::new();
    for v in rows {
        let record: WorkspaceRecord =
            serde_json::from_value(v).map_err(|e| lb_store::StoreError::Decode(e.to_string()))?;
        if record.status != WorkspaceStatus::Active {
            continue;
        }
        if !is_effective_member(store, &record.ws, sub).await? {
            continue;
        }
        out.push(IdentityWorkspace {
            ws: record.ws,
            name: record.name,
        });
    }
    out.sort_by(|a, b| a.ws.cmp(&b.ws));
    Ok(out)
}
