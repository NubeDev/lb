//! `relay_outbox` — deliver a workspace's pending effects at-least-once with retry (outbox scope,
//! vision §3 step 8). The durability backstop: a function over the `pending` set, holding no state.
//!
//! One pass at logical time `now`: scan `due` (schedulable AND past their backoff gate), deliver
//! each through the [`Target`], and record the outcome — `mark_delivered` on ack, `mark_failed`
//! otherwise. `mark_failed` applies backoff (push the next retry out) and dead-letters an effect
//! that has exhausted `max_attempts` (a poison message stops retrying — the outbox scope's deferred
//! backoff/dead-letter question, now answered). Kill the relay mid-pass and the next pass resumes
//! from the same durable set (an unmarked effect is still owed); re-delivery is safe because the
//! target dedups on `idempotency_key`. So an effect is **never lost and never double-sent**, and a
//! perpetually-failing one is parked rather than retried forever.
//!
//! Workspace-scoped: the relay runs per workspace; `due`/`mark_*` select the namespace, so a
//! ws-B relay never delivers a ws-A effect (the hard wall, §7). One hub relay at S6 (multi-relay
//! atomic claim is deferred, outbox scope).

use lb_outbox::{due, mark_delivered, mark_failed, EffectStatus};
use lb_store::{Store, StoreError};

use super::target::Target;

/// The outcome of one relay pass: how many effects were delivered, left failed (will retry after
/// backoff), or dead-lettered (exhausted `max_attempts`, parked — terminal).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RelayPass {
    pub delivered: usize,
    pub failed: usize,
    pub dead_lettered: usize,
}

/// Run one delivery pass over workspace `ws`'s **due** effects at logical time `now`, delivering
/// through `target`. Returns the pass tally. Call it again (with a later `now`) to retry whatever
/// stayed `failed` once its backoff elapses — each pass is idempotent. An effect waiting out its
/// backoff is simply not due yet, so a tight retry loop does not hammer a down target.
pub async fn relay_outbox<T: Target>(
    store: &Store,
    ws: &str,
    target: &T,
    now: u64,
) -> Result<RelayPass, StoreError> {
    let mut pass = RelayPass::default();
    for effect in due(store, ws, now).await? {
        match target.deliver(&effect).await {
            Ok(()) => {
                mark_delivered(store, ws, &effect.id).await?;
                pass.delivered += 1;
            }
            Err(_reason) => {
                // Failed: backoff + maybe dead-letter, recorded in one place by `mark_failed`.
                match mark_failed(store, ws, &effect.id, now).await? {
                    EffectStatus::DeadLettered => pass.dead_lettered += 1,
                    _ => pass.failed += 1,
                }
            }
        }
    }
    Ok(pass)
}
