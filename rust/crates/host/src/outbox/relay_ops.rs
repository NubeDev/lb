//! The **sidecar-drivable relay** verbs — the outbox surface a native driver (Tier-2 sidecar) uses to
//! deliver its OWN effects out-of-process. Before these, the outbox had only `enqueue` (stage) and
//! `status` (read): a sidecar could stage a must-deliver effect but nothing could deliver it, because
//! `relay_outbox`/`due`/`mark_*` are store-side and a sidecar holds no store handle (rule 4). These
//! three verbs expose the relay loop over the MCP bridge so a driver can run its own `Target` against
//! its own effects — symmetric with how `github-workflow` drives `relay_outbox` in-process.
//!
//! The delivery seam still lives in the driver (it owns the protocol client); the host owns only the
//! durable scan + the mark bookkeeping (the invariant: an effect is never lost and never double-sent).
//! So a native relay is: `outbox.due {target}` → deliver each through the driver's client →
//! `outbox.mark_delivered` / `outbox.mark_failed`.
//!
//! **Authorization** is per-verb (`mcp:outbox.due:call`, `mcp:outbox.mark_delivered:call`,
//! `mcp:outbox.mark_failed:call`), workspace-first (§7) — relay-operator caps only a driver/target
//! service holds; a normal caller still gets only enqueue/status. **Workspace isolation** is the wall:
//! `due`/`mark_*` select the namespace from `ws`, so a ws-B relay can physically only see/mark ws-B
//! effects. The optional `target` filter narrows *within* the workspace, so a ROS relay pulls only
//! `ros`-targeted effects (never another target's).

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_outbox::{due, mark_delivered, mark_failed, Effect};
use lb_store::Store;

use super::error::OutboxError;

/// `outbox.due {target?, now}` — the schedulable effects (pending or failed, past their backoff gate)
/// for this workspace at logical `now`, optionally filtered to one `target`. This is what a native
/// relay attempts this pass. Ordered oldest→newest (the `due` contract).
pub async fn outbox_due(
    store: &Store,
    principal: &Principal,
    ws: &str,
    target: Option<&str>,
    now: u64,
) -> Result<Vec<Effect>, OutboxError> {
    authorize_tool(principal, ws, "outbox.due").map_err(|_| OutboxError::Denied)?;
    let mut effects = due(store, ws, now).await?;
    if let Some(t) = target {
        effects.retain(|e| e.target == t);
    }
    Ok(effects)
}

/// `outbox.mark_delivered {id}` — the target acknowledged delivery of effect `id`; mark it terminal so
/// no later pass re-sends it. The driver calls this after its client confirms the external effect.
pub async fn outbox_mark_delivered(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<(), OutboxError> {
    authorize_tool(principal, ws, "outbox.mark_delivered").map_err(|_| OutboxError::Denied)?;
    mark_delivered(store, ws, id).await?;
    Ok(())
}

/// `outbox.mark_failed {id, now}` — record a failed delivery attempt of effect `id` at logical `now`:
/// count the attempt, then back off (push `next_attempt_ts`) or dead-letter (at `max_attempts`).
/// Returns the resulting status so the relay can tally dead-letters without a re-read.
pub async fn outbox_mark_failed(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    now: u64,
) -> Result<lb_outbox::EffectStatus, OutboxError> {
    authorize_tool(principal, ws, "outbox.mark_failed").map_err(|_| OutboxError::Denied)?;
    Ok(mark_failed(store, ws, id, now).await?)
}
