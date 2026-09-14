//! `case_policy_sla_delete` — erase a service-policy row over the capability gate
//! (case-plane scope §Wave 5).
//!
//! **Its own cap** (`mcp:policy.sla.delete:call`, admin) for the reason `party.delete` has one: it
//! is the destructive half, and the distinction is meaningful. In the admin bundle, so the
//! dispatcher's default convention derives it with no `tool_gate.rs` arm.

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::CaseSvcError;

/// Erase the policy at `(ws, id)`. `false` ⇒ there was no such row. Gated by
/// `mcp:policy.sla.set:call`.
pub async fn case_policy_sla_delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<bool, CaseSvcError> {
    authorize_tool(principal, ws, "policy.sla.delete").map_err(|_| CaseSvcError::Denied)?;
    Ok(lb_cases::policy_delete(store, ws, id).await?)
}
