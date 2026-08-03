//! `membership.list` — the workspace's roster (global-identity scope, decision #9: the People-tab
//! source). Gated by `mcp:members.manage:call`, workspace-first. Returns the workspace's `membership`
//! rows — the **one** source of truth. The legacy `user:*` union (lazy migration, decision #10) was
//! removed in the pre-production legacy sweep: it keyed rows by the BARE handle while the login path
//! (`identity.workspaces`) read them by the `user:`-prefixed sub, so the roster and the login path
//! could disagree (`docs/debugging/app/roster-login-disagree-legacy-user-rows.md`). Each row carries
//! the identity's display name when one is resolved. Lazy-creates `identity:{sub}` on first touch.

use lb_auth::Principal;
use lb_authz as raw;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::MembershipError;
use super::model::MembershipView;

/// The members of `ws` as `principal` sees them, sorted by `sub`.
pub async fn membership_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<MembershipView>, MembershipError> {
    authorize_tool(principal, ws, "members.manage").map_err(|_| MembershipError::Denied)?;
    let mut out = Vec::new();
    for m in raw::membership_list(store, ws).await? {
        ensure_identity(store, &m.sub).await;
        let display = raw::identity_get(store, &m.sub)
            .await?
            .and_then(|i| i.display_name);
        out.push(MembershipView {
            sub: m.sub,
            joined_ts: m.joined_ts,
            display_name: display,
        });
    }
    // One row per membership record, sorted by sub (the People tab renders this order). The dedup map
    // this used to build existed only to merge the legacy `user:*` union — with one source there is
    // nothing to merge.
    out.sort_by(|a, b| a.sub.cmp(&b.sub));
    Ok(out)
}

/// Lazy-create `identity:{sub}` if absent — best-effort; a write error never breaks
/// the list.
async fn ensure_identity(store: &Store, sub: &str) {
    if raw::identity_get(store, sub)
        .await
        .map(|o| o.is_none())
        .unwrap_or(false)
    {
        let _ = raw::identity_create(store, sub, None, 0).await;
    }
}
