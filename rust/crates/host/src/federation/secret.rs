//! DSN secret mediation (datasources scope, §6.7). The DSN for a federated source is stored,
//! read, and deleted under ONE stable principal — the federation extension itself
//! (`ext:federation`) — NEVER the (varying) admin caller who happens to run the CRUD verb. That
//! single-owner invariant is what makes datasource CRUD collision-free: any admin (the boot seed,
//! a dev login, a future IdP user) may add/update/remove a source, and the secret's owner never
//! changes, so the owner wall (secrets gate 3) never denies a legitimate overwrite or delete.
//!
//! The value is handed to the sidecar's pool and lives nowhere a rule, the page, a record, or a
//! log can observe it (the mediation invariant). The DSN never returns to the caller.

use lb_auth::Principal;
use lb_secrets::{
    delete as secret_delete, get as secret_get, reclaim as secret_reclaim, Visibility,
};

use super::error::FederationError;
use super::net::FEDERATION_EXT;
use crate::boot::Node;

/// The stable subject that OWNS every federation DSN secret — the extension principal, not the
/// admin caller. Read/write/delete of a DSN all run as this subject, so the owner wall is a no-op
/// between successive admins.
fn mediator_sub() -> String {
    format!("ext:{FEDERATION_EXT}")
}

/// The READ mediator: `ext:federation` carrying the install's REAL granted set (which holds
/// `secret:federation/*:get`). Requires the federation extension to be installed in `ws` — if it is
/// not, the source is unusable and the read is denied (the correct opaque posture: no install, no
/// runtime, no DSN). Faithful to "the extension's own authority", never the caller's.
async fn read_mediator(node: &Node, ws: &str) -> Result<Principal, FederationError> {
    let caps = lb_assets::read_install(&node.store, ws, FEDERATION_EXT)
        .await?
        .map(|i| i.granted)
        .ok_or(FederationError::Denied)?;
    Ok(Principal::routed(mediator_sub(), ws.to_string(), caps))
}

/// The WRITE/DELETE mediator: `ext:federation` with exactly the caps needed to manage the
/// extension's OWN pool secret (`secret:federation/*:write|get`). Host-constructed — it does NOT
/// depend on the install record, because managing the extension's own secret is not gated on the
/// grant record existing (registering a source can precede/accompany the install). Legitimate: the
/// CALLER already passed the `datasource.*` capability gate at the verb boundary, and the value never
/// crosses back to the caller. The workspace stays the hard wall (`ws`).
fn write_mediator(ws: &str) -> Principal {
    Principal::routed(
        mediator_sub(),
        ws.to_string(),
        vec![
            "secret:federation/*:write".to_string(),
            "secret:federation/*:get".to_string(),
        ],
    )
}

/// Read the DSN at `secret_ref` (e.g. `federation/timescale`) in `ws`, authorized as `ext:federation`
/// (the install grant holds `secret:federation/*:get`). The returned string is for the pool only —
/// the caller of `federation.query`/`datasource.test` never sees it.
pub async fn mediate_dsn(
    node: &Node,
    ws: &str,
    secret_ref: &str,
) -> Result<String, FederationError> {
    let m = read_mediator(node, ws).await?;
    secret_get(&node.store, &m, ws, secret_ref)
        .await
        .map_err(|e| match e {
            // No secret at this ref — the source was registered without a DSN (or it was forgotten).
            // Actionable, not a capability deny.
            lb_secrets::SecretsError::NotFound => FederationError::SecretUnavailable,
            _ => FederationError::Denied,
        })
}

/// Store `dsn` at `secret_ref` in `ws`, OWNED by `ext:federation` (not the admin caller). Uses
/// `reclaim`, so it HEALS a record left by an earlier bootstrap principal (a store seeded before this
/// single-owner invariant existed) — after one `add`/update the DSN is canonically owned by
/// `ext:federation`, and every later overwrite/delete passes the owner wall. Visibility is
/// `Workspace` so the mediated read passes gate 3 regardless.
pub async fn store_dsn(
    node: &Node,
    ws: &str,
    secret_ref: &str,
    dsn: &str,
) -> Result<(), FederationError> {
    let m = write_mediator(ws);
    secret_reclaim(&node.store, &m, ws, secret_ref, dsn, Visibility::Workspace)
        .await
        .map_err(|_| FederationError::Denied)
}

/// Delete the DSN at `secret_ref` in `ws` (owner = `ext:federation`, so the delete always passes the
/// owner wall). Idempotent at the store layer; a missing secret is a benign no-op so `remove` never
/// fails on an already-clean source.
pub async fn forget_dsn(node: &Node, ws: &str, secret_ref: &str) -> Result<(), FederationError> {
    let m = write_mediator(ws);
    match secret_delete(&node.store, &m, ws, secret_ref).await {
        Ok(()) => Ok(()),
        // A source registered before it ever had a DSN (or an already-forgotten secret) has no
        // record to erase — treat as done, not an error.
        Err(lb_secrets::SecretsError::NotFound) => Ok(()),
        Err(_) => Err(FederationError::Denied),
    }
}
