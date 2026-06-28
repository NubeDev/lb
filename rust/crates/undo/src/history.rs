//! `history.list` and `history.compensations` — the read side of the stack, for a UI affordance
//! (`docs/scope/undo/undo-scope.md` MCP surface). Reads only; the live stack is **state**, returned
//! as a list (not a stream — the stack is state, not motion).

use lb_store::Store;
use serde::{Deserialize, Serialize};

use crate::error::UndoError;
use crate::model::{Class, JournalEntry};
use crate::persist::{load_entry, load_stack};

/// One row of the history list, for the UI: the step plus whether it is undoable now and any
/// compensation it offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryItem {
    pub seq: u64,
    pub tool: String,
    pub class: Class,
    /// True if `undo()` would attempt this step (it is reversible). False = greyed
    /// "external — not undoable".
    pub undoable: bool,
    /// True if this step is currently on the redo side (already undone).
    pub redoable: bool,
    pub ts: u64,
}

/// List the actor's stack newest-first: undoable steps then (already-undone) redoable steps.
pub async fn list(
    store: &Store,
    ws: &str,
    actor: &str,
    surface: &str,
) -> Result<Vec<HistoryItem>, UndoError> {
    let stack = load_stack(store, ws, actor, surface).await?;
    let mut items = Vec::new();
    // Undoable, newest-first.
    for &seq in stack.undoable.iter().rev() {
        if let Some(e) = load_entry(store, ws, seq).await? {
            items.push(to_item(&e, true, false));
        }
    }
    // Redoable (already undone), newest-first.
    for &seq in stack.redoable.iter().rev() {
        if let Some(e) = load_entry(store, ws, seq).await? {
            items.push(to_item(&e, false, true));
        }
    }
    Ok(items)
}

/// The compensating tool a non-undoable step offers, if any (`Class::Compensable`). Empty for a
/// reversible or plainly-irreversible step.
pub async fn compensations(store: &Store, ws: &str, seq: u64) -> Result<Option<String>, UndoError> {
    let entry = load_entry(store, ws, seq)
        .await?
        .ok_or(UndoError::NoSuchStep)?;
    Ok(match entry.class {
        Class::Compensable { compensation_tool } => Some(compensation_tool),
        _ => None,
    })
}

fn to_item(e: &JournalEntry, on_undo_side: bool, on_redo_side: bool) -> HistoryItem {
    HistoryItem {
        seq: e.seq,
        tool: e.tool.clone(),
        class: e.class.clone(),
        undoable: on_undo_side && e.class.is_undoable(),
        redoable: on_redo_side,
        ts: e.ts,
    }
}
