//! `identity.create` — provision a global identity in the system directory (in NO workspace).
//! Gated by `mcp:identity.manage:call` (hub-only, decision #8). Idempotent on `sub` — re-creating
//! upserts the display name / created_ts. Provisioning ≠ joining (decision #4): a freshly-created
//! identity has zero memberships and cannot mint until `membership.add` (or `create_workspace`).

use lb_auth::Principal;
use lb_authz as raw;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::IdentityError;
use super::model::IdentityView;

/// Create (or update) global identity `sub` as `principal`. Returns the secret-free view.
pub async fn identity_create(
    store: &Store,
    principal: &Principal,
    sub: &str,
    display_name: Option<&str>,
    ts: u64,
) -> Result<IdentityView, IdentityError> {
    authorize_tool(principal, principal.ws(), "identity.manage")
        .map_err(|_| IdentityError::Denied)?;
    let identity = raw::identity_create(store, sub, display_name, ts).await?;
    Ok(IdentityView::from(identity))
}
