//! The asset capability gate — gate 1 (workspace) + gate 2 (capability), via the shared
//! `caps::check` chokepoint (capability-first, §3.5). This is the SAME check every surface
//! uses; assets are not special. Gate 3 (membership / grant) is separate (`visibility.rs`),
//! because a capability says "this actor may use the doc/skill surface" while membership says
//! "this actor may see *this* asset" (files + skills scopes).
//!
//! Resources: `doc/{id}` and `skill/{id}` under the `store` surface — so a held
//! `store:doc/*:read` / `store:skill/*:read` (or `:write`) grants the surface, per the
//! auth-caps grammar (which already supports it — no grammar change).

use lb_auth::Principal;
use lb_caps::{check, Action, Decision, Request, Surface};

use super::error::AssetError;

/// Authorize `action` on the doc surface for doc `id` in workspace `ws`. `Ok(())` only if gate
/// 1 (ws) and gate 2 (`store:doc/{id}:{action}`) both pass. Membership is checked separately.
pub fn authorize_doc(
    principal: &Principal,
    ws: &str,
    id: &str,
    action: Action,
) -> Result<(), AssetError> {
    gate(principal, ws, &format!("doc/{id}"), action)
}

/// Authorize `action` on the skill surface for skill `id` in workspace `ws`. The grant gate is
/// checked separately (`load_skill`).
pub fn authorize_skill(
    principal: &Principal,
    ws: &str,
    id: &str,
    action: Action,
) -> Result<(), AssetError> {
    gate(principal, ws, &format!("skill/{id}"), action)
}

/// Authorize `action` on the binary-asset surface for asset `id` in workspace `ws`
/// (document-store scope). `Ok(())` only if gate 1 (ws) and gate 2
/// (`store:asset/{id}:{action}`) both pass. Membership is checked separately (`may_read_asset`).
pub fn authorize_asset(
    principal: &Principal,
    ws: &str,
    id: &str,
    action: Action,
) -> Result<(), AssetError> {
    gate(principal, ws, &format!("asset/{id}"), action)
}

fn gate(principal: &Principal, ws: &str, resource: &str, action: Action) -> Result<(), AssetError> {
    let req = Request::new(ws, Surface::Store, resource, action);
    match check(principal, &req) {
        Decision::Allowed => Ok(()),
        Decision::Denied(_) => Err(AssetError::Denied),
    }
}
