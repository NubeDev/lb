//! The undo service error. `Denied` is opaque (§3.5) like every other service. A *stale* refusal
//! and a *not-undoable* refusal are distinct, surfaced outcomes (a UI shows "the document changed"
//! or offers a compensation) — not denials and not backend failures.

use lb_undo::UndoError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UndoSvcError {
    /// Authorization failed (workspace isolation, the verb's own cap, the original tool's cap for
    /// no-escalation, or `undo.any` for another actor). Opaque by design.
    #[error("denied")]
    Denied,
    /// Nothing to undo/redo.
    #[error("nothing to {0}")]
    Empty(&'static str),
    /// The step is not undoable (irreversible). Carries any declared compensation to offer.
    #[error("not undoable")]
    NotUndoable { compensation_tool: Option<String> },
    /// The conditional restore was refused — the record changed since the step.
    #[error("the record changed since this step — undo refused")]
    Stale,
    /// The underlying journal/store error.
    #[error("undo error: {0}")]
    Undo(UndoError),
}

impl From<UndoError> for UndoSvcError {
    fn from(e: UndoError) -> Self {
        match e {
            UndoError::Empty(w) => UndoSvcError::Empty(w),
            UndoError::Stale => UndoSvcError::Stale,
            UndoError::NotUndoable { compensation_tool } => {
                UndoSvcError::NotUndoable { compensation_tool }
            }
            other => UndoSvcError::Undo(other),
        }
    }
}
