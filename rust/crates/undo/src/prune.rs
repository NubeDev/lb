//! Persist a pushed stack cursor AND delete the journal events that fell off the depth cap — in
//! ONE store transaction (undo-exposure scope: "prune on push, no background sweeper").
//!
//! `StackState::push_do` trims the cursor past the depth cap and reports the fallen-off seqs; this
//! verb commits the trimmed cursor together with the deletion of each pruned `undo:{seq}` event and
//! its `undo_live:{seq}` companion. One transaction means there is no window where the cursor and
//! the events disagree (a crash mid-prune can never leave an event the cursor no longer reaches, or
//! a cursor pointing at a deleted event), and a concurrent in-flight undo of a seq being pruned
//! resolves to the existing typed "no such journal step" error — never a half-state.
//!
//! The stack UPSERT mirrors `lb_store::write`'s SurrealQL exactly (same envelope, same server-side
//! monotonic `rev` bump) so a pruning save is indistinguishable from a plain one to every reader.

use serde_json::Value;

use lb_store::Store;

use crate::error::UndoError;
use crate::model::{StackState, ENTRY_TABLE, STACK_TABLE};
use crate::persist::{save_stack, stack_id, LIVE_TABLE};

/// Persist `stack`, deleting the `pruned` seqs' journal events + live companions atomically with
/// it. With nothing pruned this is a plain `save_stack` (the common case — no transaction cost).
pub(crate) async fn save_stack_pruning(
    store: &Store,
    ws: &str,
    stack: &StackState,
    pruned: &[u64],
) -> Result<(), UndoError> {
    if pruned.is_empty() {
        return save_stack(store, ws, stack).await;
    }

    let value = serde_json::to_value(stack).map_err(UndoError::codec)?;
    let mut q = String::from(
        "BEGIN TRANSACTION;\n\
         UPSERT type::thing($stb, $sid) CONTENT { \
            data: $sdata, \
            rev: (type::thing($stb, $sid).rev ?? 0) + 1 \
         } RETURN NONE;\n",
    );
    let mut binds: Vec<(String, Value)> = vec![
        ("stb".into(), Value::String(STACK_TABLE.into())),
        (
            "sid".into(),
            Value::String(stack_id(&stack.actor, &stack.surface)),
        ),
        ("sdata".into(), value),
        ("etb".into(), Value::String(ENTRY_TABLE.into())),
        ("ltb".into(), Value::String(LIVE_TABLE.into())),
    ];
    for (i, seq) in pruned.iter().enumerate() {
        let key = format!("p{i}");
        binds.push((key.clone(), Value::String(seq.to_string())));
        q.push_str(&format!(
            "DELETE type::thing($etb, ${key});\nDELETE type::thing($ltb, ${key});\n"
        ));
    }
    q.push_str("COMMIT TRANSACTION;");

    store.query_ws(ws, &q, binds).await?;
    Ok(())
}
