//! `outbox_status` — the read-only delivery snapshot for a workspace (collaboration scope, slice 4).
//!
//! Gated by `mcp:outbox.status:call` (workspace-first §7). Returns the effects grouped by lifecycle
//! stage so the UI can render "pending → delivered (→ dead-letter)". Pure read — no mutation path
//! exists on this surface (the relay owns delivery). Workspace-scoped: each scan selects the
//! namespace from `ws`, so a ws-B status can physically only see ws-B effects.

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_outbox::{dead_lettered, delivered, held, pending, Effect};
use lb_store::Store;

use super::error::OutboxError;

/// A workspace's outbox snapshot, grouped by delivery lifecycle. The UI renders these three lists.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OutboxStatus {
    /// Schedulable effects not yet acknowledged (pending or failed-and-owed).
    pub pending: Vec<Effect>,
    /// Effects the target acknowledged — the completed end of the lifecycle.
    pub delivered: Vec<Effect>,
    /// Poison effects parked after exhausting their retries (terminal).
    pub dead_lettered: Vec<Effect>,
    /// Effects staged but **gated on a human approval** — proposed via `inbox.request_approval`,
    /// awaiting sign-off, NOT yet deliverable (rules-approvals scope). The reviewer sees exactly what
    /// approving will send. `#[serde(default)]` so an older client omitting the field still decodes.
    #[serde(default)]
    pub held: Vec<Effect>,
}

/// Read the outbox status for workspace `ws` as `principal`. Read-only — no effect is mutated.
pub async fn outbox_status(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<OutboxStatus, OutboxError> {
    authorize_tool(principal, ws, "outbox.status").map_err(|_| OutboxError::Denied)?;
    Ok(OutboxStatus {
        pending: pending(store, ws).await?,
        delivered: delivered(store, ws).await?,
        dead_lettered: dead_lettered(store, ws).await?,
        held: held(store, ws).await?,
    })
}
