//! `grants.assign` / `grants.revoke` / `grants.list` — the admin verbs over the durable grant store
//! (authz-grants scope). Each is gated by the admin cap `mcp:grants.assign:call` (assign/revoke
//! share it — both mutate grants) / `mcp:grants.list:call`, workspace-first through `authorize_tool`.
//!
//! **Entity-scoped grants** (entity-scoped-grants scope): `grants.assign` / `grants.revoke` accept
//! an optional `scope` selector. A scoped grant narrows the cap's reach to a subset of a table's
//! rows. `grants.list` returns the full grant records (with scope) so the Access console shows
//! selectors. The no-widening rule applies regardless of scope.

use lb_auth::Principal;
use lb_authz::{
    grant_assign_scoped, grant_list, grant_list_scoped, grant_revoke_scoped, Grant, Scope, Subject,
};
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::AuthzError;
use super::hold::holds_cap;

/// Assign `cap` to `subject` in `ws` with `scope` (entity-scoped-grants scope). Gated by
/// `mcp:grants.assign:call`; a plain cap also requires the assigner to hold it (no widening).
/// Idempotent. `Scope::All` = today's behaviour.
///
/// Selector ids are **opaque, workspace-namespace-relative** — no write-time existence check
/// (records may legitimately not exist yet, and the core can't interpret an extension's table).
/// Workspace isolation is structural: the grant row lives under `ws`, and resolution reads only
/// the caller's workspace, so a selector can never confer cross-workspace reach (see the
/// isolation decision in `docs/scope/auth-caps/entity-scoped-grants-scope.md`).
pub async fn grants_assign(
    store: &Store,
    principal: &Principal,
    ws: &str,
    subject: &Subject,
    cap: &str,
    scope: &Scope,
) -> Result<(), AuthzError> {
    authorize_tool(principal, ws, "grants.assign").map_err(|_| AuthzError::Denied)?;
    if !cap.starts_with("role:") && !holds_cap(principal, ws, cap) {
        return Err(AuthzError::Widen(cap.to_string()));
    }
    grant_assign_scoped(store, ws, subject, cap, scope).await?;
    Ok(())
}

/// Revoke `cap` with `scope` from `subject` in `ws`. Gated by `mcp:grants.assign:call` (the same
/// mutate cap). Idempotent; revoking an absent grant is a success.
pub async fn grants_revoke(
    store: &Store,
    principal: &Principal,
    ws: &str,
    subject: &Subject,
    cap: &str,
    scope: &Scope,
) -> Result<(), AuthzError> {
    authorize_tool(principal, ws, "grants.assign").map_err(|_| AuthzError::Denied)?;
    grant_revoke_scoped(store, ws, subject, cap, scope).await?;
    Ok(())
}

/// List the live caps `subject` holds directly in `ws` (caps only, no scope). Gated by
/// `mcp:grants.list:call`.
pub async fn grants_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
    subject: &Subject,
) -> Result<Vec<String>, AuthzError> {
    authorize_tool(principal, ws, "grants.list").map_err(|_| AuthzError::Denied)?;
    Ok(grant_list(store, ws, subject).await?)
}

/// List the live grants `subject` holds directly in `ws` — the full records including scope
/// (entity-scoped-grants scope). Gated by `mcp:grants.list:call`. The Access console uses this to
/// show selectors.
pub async fn grants_list_scoped(
    store: &Store,
    principal: &Principal,
    ws: &str,
    subject: &Subject,
) -> Result<Vec<Grant>, AuthzError> {
    authorize_tool(principal, ws, "grants.list").map_err(|_| AuthzError::Denied)?;
    Ok(grant_list_scoped(store, ws, subject).await?)
}
