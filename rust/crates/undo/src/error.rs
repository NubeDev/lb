//! Errors for the undo journal. Distinct refusal reasons matter here: a *stale* refusal (the
//! record changed since the step) is a normal, expected outcome the UI surfaces ("the document
//! changed — undo refused"), not a backend failure — so it is its own variant.

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UndoError {
    #[error("store error: {0}")]
    Store(#[from] StoreError),

    #[error("value did not (de)serialize: {0}")]
    Codec(String),

    /// Nothing to undo/redo on this stack.
    #[error("nothing to {0}")]
    Empty(&'static str),

    /// The action is not undoable (irreversible — reached the outbox). Carries the optional
    /// compensation tool to offer instead (set for [`crate::Class::Compensable`]).
    #[error("action is not undoable (irreversible)")]
    NotUndoable { compensation_tool: Option<String> },

    /// The conditional-restore predicate failed: a touched record's current `rev` no longer
    /// matches what the step expects (an intervening writer). Undo/redo **declines** rather than
    /// clobbering — the scope's safe-by-refusal guarantee.
    #[error("the record changed since this step — undo refused")]
    Stale,

    /// The referenced step `seq` is not in the journal (pruned or never existed).
    #[error("no such journal step")]
    NoSuchStep,
}

impl UndoError {
    pub(crate) fn codec(e: impl std::fmt::Display) -> Self {
        UndoError::Codec(e.to_string())
    }
}
