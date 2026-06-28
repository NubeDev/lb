//! Enqueue an effect **transactionally** with the domain change that justified it — the verb that
//! makes the outbox a durability backstop rather than best-effort pub/sub (outbox scope, README
//! §6.10).
//!
//! `change` is the caller's domain record (a job step, a doc, an inbox resolution); `effect` is the
//! must-deliver intent. Both are upserted in ONE SurrealDB transaction (`lb_store::write_tx`): they
//! commit together or roll back together. So there is no window where the change is durable but the
//! effect is lost (an effect that never gets relayed), nor where the effect exists for a change that
//! never landed (a phantom delivery). The caller's retry re-runs the whole transaction.
//!
//! Raw verb — the host `workflow` service runs `caps::check` before this (capability-first §3.5).

use lb_store::{mark_outbox_reached, write_tx, Store, StoreError, Upsert};

use super::model::Effect;
use super::TABLE;

/// Atomically write `change` (a domain record at `change_table:change_id`) AND `effect` (the
/// must-deliver intent at `outbox:{effect.id}`) into workspace `ws`. Idempotent on both ids. If
/// either upsert fails the transaction rolls back — neither lands.
pub async fn enqueue(
    store: &Store,
    ws: &str,
    change_table: &str,
    change_id: &str,
    change: &serde_json::Value,
    effect: &Effect,
) -> Result<(), StoreError> {
    let effect_value =
        serde_json::to_value(effect).map_err(|e| StoreError::Decode(e.to_string()))?;
    write_tx(
        store,
        ws,
        &Upsert {
            table: change_table,
            id: change_id,
            value: change,
        },
        &Upsert {
            table: TABLE,
            id: &effect.id,
            value: &effect_value,
        },
    )
    .await?;
    // Taint the in-flight tool call: its transaction reached the outbox (irreversible motion,
    // §6.10). The undo dispatch seam reads this to classify the action `irreversible` from what it
    // actually did — derived, never trusted from a manifest (undo scope "runtime transaction
    // taint"). A no-op outside a dispatch taint scope, so non-dispatch callers are unaffected.
    mark_outbox_reached();
    Ok(())
}
