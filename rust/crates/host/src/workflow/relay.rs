//! `relay_outbox` — deliver a workspace's pending effects at-least-once with retry (outbox scope,
//! vision §3 step 8). The durability backstop: a function over the `pending` set, holding no state.
//!
//! One pass: scan `pending` (status `pending` or `failed`), deliver each through the [`Target`], and
//! record the outcome — `mark_delivered` on ack, `mark_failed` (stays schedulable) on a transient
//! failure. Kill the relay mid-pass and the next pass resumes from the same durable `pending` set
//! (an unmarked effect is still pending); re-delivery is safe because the target dedups on
//! `idempotency_key`. So an effect is **never lost and never double-sent** — the at-least-once
//! guarantee the S6 exit gate requires.
//!
//! Workspace-scoped: the relay runs per workspace; `pending`/`mark_*` select the namespace, so a
//! ws-B relay never delivers a ws-A effect (the hard wall, §7). One hub relay at S6 (multi-relay
//! atomic claim is deferred, outbox scope).

use lb_outbox::{mark_delivered, mark_failed, pending};
use lb_store::{Store, StoreError};

use super::target::Target;

/// The outcome of one relay pass: how many effects were delivered vs left failed (for retry).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RelayPass {
    pub delivered: usize,
    pub failed: usize,
}

/// Run one delivery pass over workspace `ws`'s pending effects, delivering through `target`. Returns
/// the pass tally. Call it again to retry whatever stayed `failed` — each pass is idempotent.
pub async fn relay_outbox<T: Target>(
    store: &Store,
    ws: &str,
    target: &T,
) -> Result<RelayPass, StoreError> {
    let mut pass = RelayPass::default();
    for effect in pending(store, ws).await? {
        match target.deliver(&effect).await {
            Ok(()) => {
                mark_delivered(store, ws, &effect.id).await?;
                pass.delivered += 1;
            }
            Err(_reason) => {
                // Transient: leave it schedulable, count the attempt, retry next pass.
                mark_failed(store, ws, &effect.id).await?;
                pass.failed += 1;
            }
        }
    }
    Ok(pass)
}
