//! `identity.workspaces` — the workspaces an identity is a member of (global-identity scope, decision
//! #5). A **hub-only bounded scan**: reads the node's workspace directory, then for each workspace
//! checks its `membership` table for `sub`. This is NOT a per-request hot path — it runs once at login
//! and when the switcher opens (decision #5). The per-workspace `membership` table IS the index; no
//! denormalized reverse index in v1 (a second source of truth would drift).
//!
//! Membership is the ONE source of truth: a `membership` row and nothing else. The legacy
//! `user:<sub>` fallback (decision #10's lazy migration) was removed in the pre-production legacy
//! sweep — it disagreed with `membership.list`'s own legacy pass on the key (keyed sub here, bare
//! handle there), so a person could be on the roster and still fail login. The identity record is
//! created lazily on first touch below; this verb only resolves membership.

use lb_auth::Principal;
use lb_authz as raw;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::IdentityError;
use super::model::IdentityWorkspace;
use crate::workspaces::{
    WorkspaceRecord, WorkspaceStatus, KIND as WS_KIND, TABLE as WS_TABLE, WORKSPACES_NS,
};

/// The workspaces `sub` is an effective member of, for `principal`. Each row carries the workspace id
/// + display name. Drives the login picker + the switcher. Bounded by the directory size (a hub hosts
///   few workspaces — decision #5).
pub async fn identity_workspaces(
    store: &Store,
    principal: &Principal,
    sub: &str,
) -> Result<Vec<IdentityWorkspace>, IdentityError> {
    authorize_tool(principal, principal.ws(), "identity.manage")
        .map_err(|_| IdentityError::Denied)?;
    let rows = lb_store::list(store, WORKSPACES_NS, WS_TABLE, "kind", WS_KIND).await?;
    let mut out = Vec::new();
    for v in rows {
        let record: WorkspaceRecord =
            serde_json::from_value(v).map_err(|e| lb_store::StoreError::Decode(e.to_string()))?;
        if record.status != WorkspaceStatus::Active {
            continue;
        }
        if is_effective_member(store, &record.ws, sub).await? {
            out.push(IdentityWorkspace {
                ws: record.ws,
                name: record.name,
            });
        }
    }
    // Lazy-create the identity on first resolution if absent (decision #10) — best-effort; a read
    // error never blocks resolution.
    if raw::identity_get(store, sub).await?.is_none() {
        let _ = raw::identity_create(store, sub, None, 0).await;
    }
    out.sort_by(|a, b| a.ws.cmp(&b.ws));
    Ok(out)
}

/// Is `sub` an effective member of `ws`? A live `membership` row — nothing else. The wall holds: this
/// reads `ws`'s own namespace only.
pub(crate) async fn is_effective_member(
    store: &Store,
    ws: &str,
    sub: &str,
) -> Result<bool, lb_store::StoreError> {
    raw::membership_is_member(store, ws, sub).await
}
