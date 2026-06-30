//! `membership_login_resolve` — the **login-path membership seam** (global-identity scope). Called by
//! the gateway `login` route *before* it mints, with NO principal yet (we are deciding whether to
//! create one) — so this is un-gated, like `user_login_check`. It enforces decisions #3 + #4:
//!
//! - an identity that is an **effective member** of the requested ws (membership row OR legacy user
//!   row) → mint allowed;
//! - the requested ws has **no members at all** → this is a brand-new workspace: bootstrap the
//!   requester as the first member AND grant them `workspace-admin` (decision #3 — the dev-login
//!   realization of the first-member bootstrap, preserving the auto-seed demo);
//! - the requested ws has members but NOT this sub → `NotAMember` (decision #4: a provisioned
//!   identity with zero memberships cannot mint into a workspace it has not been added to).
//!
//! Identity is lazy-created on first touch (decision #10). The bootstrap grant is a SYSTEM effect via
//! the raw `grant_assign` (not the gated host verb) — the same reasoning as `membership.add`.

use lb_authz as raw;
use lb_store::Store;

use super::add::MEMBER_ROLE_CAP;
use super::error::MembershipError;
use crate::identity::{has_any_effective_member, is_effective_member};

/// The built-in admin role the first member of a workspace receives (decision #3).
pub const WORKSPACE_ADMIN_ROLE_CAP: &str = "role:workspace-admin";

/// Resolve the login membership for `sub` into `ws`. `Ok(())` if minting may proceed; `Err(NotAMember)`
/// if the sub is not a member of a workspace that already has one. Bootstraps an empty workspace.
pub async fn membership_login_resolve(
    store: &Store,
    ws: &str,
    sub: &str,
    ts: u64,
) -> Result<(), MembershipError> {
    // Lazy identity creation (decision #10) — best-effort.
    if raw::identity_get(store, sub).await?.is_none() {
        let _ = raw::identity_create(store, sub, None, ts).await;
    }
    if is_effective_member(store, ws, sub).await? {
        return Ok(());
    }
    if !has_any_effective_member(store, ws).await? {
        // Brand-new workspace: first login bootstraps the requester as workspace-admin (decision #3).
        bootstrap_first_member(store, ws, sub, ts).await?;
        return Ok(());
    }
    Err(MembershipError::Denied)
}

/// First-member bootstrap: write the membership row + grant `member` AND `workspace-admin`.
async fn bootstrap_first_member(
    store: &Store,
    ws: &str,
    sub: &str,
    ts: u64,
) -> Result<(), MembershipError> {
    raw::membership_add_raw(store, ws, sub, ts).await?;
    if let Some(name) = sub.strip_prefix("user:") {
        let subject = lb_authz::Subject::User(name.to_string());
        raw::grant_assign(store, ws, &subject, MEMBER_ROLE_CAP).await?;
        raw::grant_assign(store, ws, &subject, WORKSPACE_ADMIN_ROLE_CAP).await?;
    }
    Ok(())
}
