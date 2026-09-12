//! `case_party_delete` — erase a party row over the capability gate (case-plane scope §Wave 5).
//!
//! **Its own cap** (`mcp:party.delete:call`, admin), not an alias of the write. It is the
//! destructive half, and "may author the roster, may not erase from it" is a distinction a
//! workspace could reasonably want — unlike `case.merge`/`case.split`, where the alias exists
//! because the distinction would be meaningless. The cap is in the admin bundle, so the
//! dispatcher's default convention derives it and `tool_gate.rs` needs no arm.
//!
//! The refusal that matters — a party any request points at cannot be deleted — is the crate's
//! (`lb_cases::party_delete`), because it is a rule about the data rather than about the caller.

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::CaseSvcError;

/// Erase the party at `(ws, id)`. `false` ⇒ there was no such row. Gated by `mcp:party.delete:call`.
pub async fn case_party_delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<bool, CaseSvcError> {
    authorize_tool(principal, ws, "party.delete").map_err(|_| CaseSvcError::Denied)?;
    Ok(lb_cases::party_delete(store, ws, id).await?)
}
