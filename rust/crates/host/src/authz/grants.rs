//! `grants.assign` / `grants.revoke` / `grants.list` — the admin verbs over the durable grant store
//! (authz-grants scope). Each is gated by the admin cap `mcp:grants.assign:call` (assign/revoke
//! share it — both mutate grants) / `mcp:grants.list:call`, workspace-first through `authorize_tool`.
//!
//! No-widening: `grants.assign` of a *plain* cap requires the assigner to **hold that cap** — you
//! cannot grant authority you lack (authz-grants risk: "custom roles can only grant caps the
//! assigner holds"). A `role:<name>` grant is exempt here (the role's caps were already bounded at
//! `roles.define`). Assign/revoke are idempotent (the store verbs are upserts).

use lb_auth::Principal;
use lb_authz::{grant_assign, grant_list, grant_revoke, Subject};
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::AuthzError;
use super::hold::holds_cap;

/// Assign `cap` to `subject` in `ws`. Gated by `mcp:grants.assign:call`; a plain cap also requires
/// the assigner to hold it (no widening). Idempotent.
pub async fn grants_assign(
    store: &Store,
    principal: &Principal,
    ws: &str,
    subject: &Subject,
    cap: &str,
) -> Result<(), AuthzError> {
    authorize_tool(principal, ws, "grants.assign").map_err(|_| AuthzError::Denied)?;
    if !cap.starts_with("role:") && !holds_cap(principal, ws, cap) {
        return Err(AuthzError::Widen(cap.to_string()));
    }
    grant_assign(store, ws, subject, cap).await?;
    Ok(())
}

/// Revoke `cap` from `subject` in `ws`. Gated by `mcp:grants.assign:call` (the same mutate cap).
/// Idempotent; revoking an absent grant is a success.
pub async fn grants_revoke(
    store: &Store,
    principal: &Principal,
    ws: &str,
    subject: &Subject,
    cap: &str,
) -> Result<(), AuthzError> {
    authorize_tool(principal, ws, "grants.assign").map_err(|_| AuthzError::Denied)?;
    grant_revoke(store, ws, subject, cap).await?;
    Ok(())
}

/// List the live caps `subject` holds directly in `ws`. Gated by `mcp:grants.list:call`.
pub async fn grants_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
    subject: &Subject,
) -> Result<Vec<String>, AuthzError> {
    authorize_tool(principal, ws, "grants.list").map_err(|_| AuthzError::Denied)?;
    Ok(grant_list(store, ws, subject).await?)
}
