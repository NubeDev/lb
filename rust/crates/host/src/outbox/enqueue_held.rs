//! `enqueue_held_outbox` — stage a must-deliver effect in the **`held`** status: proposed by a rule
//! via `inbox.request_approval`, gated on a human approval, NOT yet deliverable (rules-approvals
//! scope).
//!
//! Identical authority + transactional path to [`enqueue_outbox`](super::enqueue_outbox) — gated by
//! `mcp:outbox.enqueue:call` (workspace-first §7), staged through `lb_outbox::enqueue`'s
//! change+effect write so the effect is never orphaned. The ONE difference is the initial status:
//! `Held` instead of `Pending`, so the relay skips it (its scan is `pending`/`failed` only — a held
//! effect is never delivered) until the approval reactor releases it (`held → pending`) on approval
//! or discards it (`held → discarded`) on rejection.
//!
//! No new capability: a rule that may stage an effect at all (`outbox.enqueue`) may stage it held —
//! the *release* is the gated step (the approval reactor's system authority), not the stage. One verb
//! per file (FILE-LAYOUT §3).

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_outbox::{enqueue, Effect};
use lb_store::Store;

use super::error::OutboxError;

/// The change table the held enqueue records its justification under (sibling to the plain enqueue's).
const CHANGE_TABLE: &str = "approval_held_change";

/// Enqueue effect `id` (target/action/payload) in workspace `ws` as `principal`, staged **`held`**.
/// `ts` is the logical timestamp; `idempotency_key` is the effect id (re-staging the same id upserts).
/// The relay never delivers a held effect — the approval reactor releases it once the matching
/// `needs:approval` item is approved.
#[allow(clippy::too_many_arguments)]
pub async fn enqueue_held_outbox(
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
    let effect = Effect::new(id, target, action, payload, id, ts).held();
    let change = serde_json::json!({ "by": principal.sub(), "effect": id, "held": true, "ts": ts });
    enqueue(store, ws, CHANGE_TABLE, id, &change, &effect).await?;
    Ok(())
}
