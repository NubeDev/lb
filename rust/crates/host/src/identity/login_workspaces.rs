//! `login_workspaces` — the **login-path** membership enumeration (email-login scope). The un-gated
//! sibling of `identity.workspaces` (which is admin-gated): `/auth/login` calls this AFTER the
//! credential verifies but BEFORE any principal exists, to build the 0/1/N branch. It returns the
//! workspaces `sub` is an EFFECTIVE member of, **minus** any where the per-ws `user_login_check`
//! refuses (disabled there ≠ disabled everywhere) — so a person disabled in `acme` but active in
//! `globex` logs in seeing only `globex`.
//!
//! Same bounded hub scan as `identity.workspaces` (a hub hosts few workspaces; this runs once at
//! login), same `{ws, name}` rows the picker renders. No authorization — pre-principal, exactly like
//! `membership_login_resolve` / the credential verify.

use lb_store::Store;

use super::error::IdentityError;
use super::model::IdentityWorkspace;
use super::workspaces::is_effective_member;
use crate::users::user_login_check;
use crate::workspaces::{
    WorkspaceRecord, WorkspaceStatus, KIND as WS_KIND, TABLE as WS_TABLE, WORKSPACES_NS,
};

/// The workspaces `sub` may log into: active workspaces it is an effective member of AND is not
/// disabled in. Sorted by `ws`. Bounded by the directory size.
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
        // Disabled-in-this-workspace drops it from the roster (mirror of the login gate). A missing
        // user record is NOT disabled (`user_login_check` returns Ok) — a fresh member still lists.
        if user_login_check(store, &record.ws, sub).await.is_err() {
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
