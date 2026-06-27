//! `roles.define` / `roles.list` — the admin verbs over custom role bundles (authz-grants scope).
//! `define` is gated by `mcp:roles.define:call`; `list` by `mcp:roles.list:call`, workspace-first.
//!
//! No-widening (the privilege-escalation guard): a `roles.define` may only bundle caps the definer
//! **holds** — otherwise a workspace-admin could mint a role granting more than they have and assign
//! it to themselves. Built-in roles (super-admin/workspace-admin/member) are seeded elsewhere and
//! not redefinable through this verb (the caller passes only custom names). Idempotent on the name.

use lb_auth::Principal;
use lb_authz::{role_define, role_list, Role};
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::AuthzError;
use super::hold::holds_cap;

/// Define (or replace) custom role `name` with `caps` in `ws`. Every cap must be one the definer
/// holds (no widening). Idempotent on the name.
pub async fn roles_define(
    store: &Store,
    principal: &Principal,
    ws: &str,
    name: &str,
    caps: &[String],
) -> Result<(), AuthzError> {
    authorize_tool(principal, ws, "roles.define").map_err(|_| AuthzError::Denied)?;
    for cap in caps {
        if !holds_cap(principal, ws, cap) {
            return Err(AuthzError::Widen(cap.clone()));
        }
    }
    role_define(store, ws, name, caps).await?;
    Ok(())
}

/// List every role defined in `ws`. Gated by `mcp:roles.list:call`.
pub async fn roles_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<Role>, AuthzError> {
    authorize_tool(principal, ws, "roles.list").map_err(|_| AuthzError::Denied)?;
    Ok(role_list(store, ws).await?)
}
