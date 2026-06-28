//! `undo()` — reverse the newest undoable step on a (ws, actor[, surface]) stack.
//!
//! The flow (`docs/scope/undo/undo-scope.md` example): peek the stack's newest undoable step;
//! **refuse** if it is not undoable (irreversible/compensable — offer the compensation); build the
//! conditional restore from its before-images + expected `after` revs; apply it (refused-if-stale);
//! journal a `kind:undo` entry (so redo can re-apply); move the cursor (undoable→redoable).
//!
//! Undo is a forward, audited action: the caller (host layer) checks the actor holds the original
//! tool's cap *before* calling this, so undo can never reach a mutation the actor couldn't perform.

use lb_store::Store;

use crate::error::UndoError;
use crate::model::{JournalEntry, Kind};
use crate::persist::{load_entry, load_stack, save_stack};
use crate::restore::{restore_all, Restore};

/// Undo the newest undoable step. Returns the reversed step's [`JournalEntry`] (the `Do`).
pub async fn apply_undo(
    store: &Store,
    ws: &str,
    actor: &str,
    surface: &str,
) -> Result<JournalEntry, UndoError> {
    let mut stack = load_stack(store, ws, actor, surface).await?;
    let seq = stack.peek_undo().ok_or(UndoError::Empty("undo"))?;
    let entry = load_entry(store, ws, seq)
        .await?
        .ok_or(UndoError::NoSuchStep)?;

    // Refuse a not-undoable step, surfacing any declared compensation.
    if !entry.class.is_undoable() {
        return Err(UndoError::NotUndoable {
            compensation_tool: match &entry.class {
                crate::model::Class::Compensable { compensation_tool } => {
                    Some(compensation_tool.clone())
                }
                _ => None,
            },
        });
    }

    // Build the conditional restore: write each record's `before`, guarded by the rev the record
    // currently sits at (the live predicate — capture-time `after` on the first undo, then whatever
    // the last undo/redo cycle left). Any intervening external writer changes this rev → refusal.
    let live = crate::persist::load_live_revs(store, ws, &entry).await?;
    let restores: Vec<Restore> = entry
        .touched
        .iter()
        .zip(live.iter())
        .map(|(t, &rev)| Restore {
            table: t.table.clone(),
            id: t.id.clone(),
            target: t.before.clone(),
            expected_rev: rev,
        })
        .collect();
    let produced = restore_all(store, ws, &restores).await?;
    // Record the rev each record now sits at, so a subsequent redo guards against it.
    crate::persist::save_live_revs(store, ws, seq, &produced).await?;

    // Journal the undo as its own immutable entry (enables redo). Its `touched` mirrors the step
    // but with before/after swapped intent is implicit via Kind::Undo; we reuse the original
    // touched data so redo can re-apply the `after`.
    let undo_seq = crate::persist::next_seq(store, ws).await?;
    let undo_entry = JournalEntry {
        seq: undo_seq,
        kind: Kind::Undo,
        group: entry.group.clone(),
        ..entry.clone()
    };
    crate::persist::save_entry(store, ws, &undo_entry).await?;

    // Move the cursor: the step is now redoable.
    stack.commit_undo();
    save_stack(store, ws, &stack).await?;

    Ok(entry)
}
