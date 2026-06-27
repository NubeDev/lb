//! `enqueue_outbox` — stage a must-deliver effect over the capability gate (proof-workflow-sim scope).
//!
//! The host-callback's first outbox WRITE that PRODUCES workflow motion: a guest (or any bridged caller)
//! enqueues a pending effect that then shows up in `outbox_status`'s `pending`. Gated by
//! `mcp:outbox.enqueue:call` (workspace-first §7). The effect is staged through `lb_outbox::enqueue`'s
//! transactional change+effect write — the change row records the justification (e.g. the approval the
//! effect followed), so the effect is never orphaned (outbox scope). Staging only: the relay owns
//! delivery, so a fresh effect is `Pending` (the durable backstop), never `Delivered` here.
//!
//! The raw transactional persistence stays in `lb_outbox::enqueue`; this layer is authorization only
//! (one verb per file, FILE-LAYOUT §3).

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_outbox::{enqueue, Effect};
use lb_store::Store;

use super::error::OutboxError;

/// The change table the enqueue records its justification under. A bridged enqueue carries no prior
/// domain change of its own, so we record a minimal justification row keyed by the effect id — enough
/// to keep `lb_outbox::enqueue`'s "effect is never orphaned from a change" invariant honest.
const CHANGE_TABLE: &str = "proof_sim_change";

/// Enqueue effect `id` (target/action/payload) in workspace `ws` as `principal`. `ts` is the logical
/// timestamp. The effect's `idempotency_key` is the effect id (re-enqueuing the same id is a no-op).
/// Staged `Pending` — delivery is the relay's job.
#[allow(clippy::too_many_arguments)]
pub async fn enqueue_outbox(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    target: &str,
    action: &str,
    payload: &str,
    ts: u64,
) -> Result<(), OutboxError> {
    authorize_tool(principal, ws, "outbox.enqueue").map_err(|_| OutboxError::Denied)?;
    let effect = Effect::new(id, target, action, payload, id, ts);
    let change = serde_json::json!({ "by": principal.sub(), "effect": id, "ts": ts });
    enqueue(store, ws, CHANGE_TABLE, id, &change, &effect).await?;
    Ok(())
}
