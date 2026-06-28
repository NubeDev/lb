//! `redo()` — re-apply the newest redoable step (the inverse of [`apply_undo`](crate::apply_undo)).
//!
//! Symmetric to undo: peek the stack's newest redoable step, build a conditional restore that
//! writes each record's `after` image guarded by the live rev (what the last undo left), apply it
//! (refused-if-stale), journal a `kind:redo` entry, and move the cursor (redoable→undoable). A new
//! `do` between an undo and a redo would have truncated the redo stack, so a redoable step always
//! reflects a still-valid future.

use lb_store::Store;

use crate::error::UndoError;
use crate::model::{JournalEntry, Kind};
use crate::persist::{load_entry, load_stack, save_stack};
use crate::restore::{restore_all, Restore};

/// Redo the newest redoable step. Returns the re-applied step's [`JournalEntry`] (the original `Do`).
pub async fn apply_redo(
    store: &Store,
    ws: &str,
    actor: &str,
    surface: &str,
) -> Result<JournalEntry, UndoError> {
    let mut stack = load_stack(store, ws, actor, surface).await?;
    let seq = stack.peek_redo().ok_or(UndoError::Empty("redo"))?;
    let entry = load_entry(store, ws, seq)
        .await?
        .ok_or(UndoError::NoSuchStep)?;

    // Build the conditional restore: write each record's `after`, guarded by the live rev (what the
    // undo left the record at). An intervening writer since the undo → refusal.
    let live = crate::persist::load_live_revs(store, ws, &entry).await?;
    let restores: Vec<Restore> = entry
        .touched
        .iter()
        .zip(live.iter())
        .map(|(t, &rev)| Restore {
            table: t.table.clone(),
            id: t.id.clone(),
            target: t.after.clone(),
            expected_rev: rev,
        })
        .collect();
    let produced = restore_all(store, ws, &restores).await?;
    crate::persist::save_live_revs(store, ws, seq, &produced).await?;

    // Journal the redo as its own immutable entry.
    let redo_seq = crate::persist::next_seq(store, ws).await?;
    let redo_entry = JournalEntry {
        seq: redo_seq,
        kind: Kind::Redo,
        group: entry.group.clone(),
        ..entry.clone()
    };
    crate::persist::save_entry(store, ws, &redo_entry).await?;

    // Move the cursor: the step is undoable again.
    stack.commit_redo();
    save_stack(store, ws, &stack).await?;

    Ok(entry)
}
