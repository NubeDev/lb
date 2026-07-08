//! `membership.add` — invite/join: add a global identity to this workspace (global-identity scope).
//! Gated by `mcp:members.manage:call`, workspace-first (ws from the token — a forged cross-workspace
//! add is denied server-side because the row lands in the caller's ws). Idempotent on `(ws, sub)`.
//!
//! On join the system grants the built-in **`member`** role to `Subject::User(sub)` (decision #2 — no
//! `role_hint` arg). This grant is a SYSTEM effect via the raw [`grant_assign`](lb_authz::grant_assign),
//! NOT the gated `grants_assign` host verb — so it lands regardless of the caller's caps and is not
//! blocked by the no-widening rule (joining is a system act, not a user widening). An admin grants
//! more through the regular grants surface.

use lb_auth::Principal;
use lb_authz as raw;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::MembershipError;

/// The built-in role granted on join (decision #2). Assigning it is an ordinary `role:member` grant.
pub const MEMBER_ROLE_CAP: &str = "role:member";

/// Add `sub` to workspace `ws` as `principal`, granting the `member` role. Idempotent. `sub` is the
/// global identity handle (`user:ada`).
pub async fn membership_add(
    store: &Store,
    principal: &Principal,
    ws: &str,
    sub: &str,
    ts: u64,
) -> Result<(), MembershipError> {
    authorize_tool(principal, ws, "members.manage").map_err(|_| MembershipError::Denied)?;
    // Seed the built-in role records so the `role:member` grant below resolves to member caps
    // (login-hardening scope). Idempotent; a joined member gets ONLY `role:member` (not admin).
    crate::authz::ensure_builtin_authz_roles(store, ws).await?;
    raw::membership_add_raw(store, ws, sub, ts).await?;
    // System-grant the built-in member role. Best-effort identity creation so the directory lists the
    // joined identity (lazy migration, decision #10) — a write error here is non-fatal to the join.
    if raw::identity_get(store, sub).await?.is_none() {
        let _ = raw::identity_create(store, sub, None, ts).await;
    }
    if let Some(name) = bare_user(sub) {
        raw::grant_assign(
            store,
            ws,
            &lb_authz::Subject::User(name.to_string()),
            MEMBER_ROLE_CAP,
        )
        .await?;
    }
    Ok(())
}

/// Strip the `user:` prefix from a `user:<name>` sub, returning the bare name a `Subject::User`
/// carries. Returns `None` for a sub that is not a user handle (a non-user subject cannot be a member
/// in v1).
fn bare_user(sub: &str) -> Option<&str> {
    sub.strip_prefix("user:")
}
