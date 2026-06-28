//! DSN secret mediation (datasources scope, §6.7). The host pulls the connection string out of
//! `lb-secrets` under the **federation extension's own grant** (`requested ∩ admin_approved`, the
//! install record) — NEVER the caller's authority and never returned to the caller. The value is
//! handed to the sidecar's pool and lives nowhere a rule, the page, a record, or a log can observe it.

use lb_auth::Principal;
use lb_secrets::get as secret_get;

use super::error::FederationError;
use super::net::FEDERATION_EXT;
use crate::boot::Node;

/// Read the DSN at `secret_ref` (e.g. `federation/tsdb`) in `ws`, authorized as the federation
/// extension (its install grant must hold `secret:federation/*:get`). The returned string is for the
/// pool only — the caller of `federation.query` never sees it.
pub async fn mediate_dsn(
    node: &Node,
    ws: &str,
    secret_ref: &str,
) -> Result<String, FederationError> {
    // The extension's effective authority for the secret read: an unconstrained principal in this
    // workspace holding exactly the federation install's granted caps. If the install is missing or
    // the secret grant was not approved, the read is denied (opaque) — the source is unusable, which
    // is the correct posture (no DSN, no connect).
    let granted = lb_assets::read_install(&node.store, ws, FEDERATION_EXT)
        .await?
        .map(|i| i.granted)
        .ok_or(FederationError::Denied)?;
    let mediator = Principal::routed(format!("ext:{FEDERATION_EXT}"), ws.to_string(), granted);

    secret_get(&node.store, &mediator, ws, secret_ref)
        .await
        .map_err(|_| FederationError::Denied)
}
