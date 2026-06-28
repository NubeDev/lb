//! Peek the next undo/redo target without applying it — so the host layer can run the
//! no-escalation capability check (does the actor hold the *original tool's* cap?) BEFORE the
//! conditional restore runs. Returns the [`JournalEntry`] the next `apply_undo`/`apply_redo` would
//! act on, or `None` if the stack side is empty.

use lb_store::Store;

use crate::error::UndoError;
use crate::model::JournalEntry;
use crate::persist::{load_entry, load_stack};

/// The entry the next [`apply_undo`](crate::apply_undo) would reverse, or `None`.
pub async fn peek_undo(
    store: &Store,
    ws: &str,
    actor: &str,
    surface: &str,
) -> Result<Option<JournalEntry>, UndoError> {
    let stack = load_stack(store, ws, actor, surface).await?;
    match stack.peek_undo() {
        Some(seq) => load_entry(store, ws, seq).await,
        None => Ok(None),
    }
}

/// The entry the next [`apply_redo`](crate::apply_redo) would re-apply, or `None`.
pub async fn peek_redo(
    store: &Store,
    ws: &str,
    actor: &str,
    surface: &str,
) -> Result<Option<JournalEntry>, UndoError> {
    let stack = load_stack(store, ws, actor, surface).await?;
    match stack.peek_redo() {
        Some(seq) => load_entry(store, ws, seq).await,
        None => Ok(None),
    }
}
