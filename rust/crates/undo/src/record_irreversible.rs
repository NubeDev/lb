//! Record a not-undoable step: an action whose transaction reached the outbox (irreversible) or
//! that declared a compensation (compensable). It lands on the stack as a **marker** — visible in
//! `history.list` (greyed "external — not undoable"), but `undo()` refuses it and offers the
//! declared compensation for the compensable case (`docs/scope/undo/undo-scope.md`).
//!
//! No before-image is captured (there is nothing reversible to restore). The marker exists so the
//! history is complete and so a *grouped* undo can refuse up front when any step in the group is
//! irreversible (the group's max class).

use lb_store::Store;

use crate::error::UndoError;
use crate::model::{Class, JournalEntry, Kind, DEFAULT_DEPTH_CAP};
use crate::persist::{load_stack, next_seq, save_entry};
use crate::prune::save_stack_pruning;

/// What a caller hands us to record an irreversible/compensable step.
pub struct RecordIrreversible<'a> {
    pub ws: &'a str,
    pub actor: &'a str,
    pub surface: &'a str,
    pub tool: &'a str,
    pub trace_id: &'a str,
    pub ts: u64,
    /// The derived class — [`Class::Irreversible`] or [`Class::Compensable`]. A
    /// [`Class::Reversible`] here is a caller bug; we still record it as a non-undoable marker
    /// (no before-image was captured), which fails safe (an over-refusal, never a silent clobber).
    pub class: Class,
    /// Optional group id (for a multi-step action). Defaults to the step's own seq.
    pub group: Option<String>,
    /// Bounded stack depth: steps that fall past this are pruned (event + live companion deleted)
    /// in the same transaction as the cursor push. `None` = [`DEFAULT_DEPTH_CAP`].
    pub depth_cap: Option<usize>,
}

/// Record the not-undoable step and push it onto the actor's stack. Returns the step `seq`.
pub async fn record_irreversible(
    store: &Store,
    rec: RecordIrreversible<'_>,
) -> Result<u64, UndoError> {
    let seq = next_seq(store, rec.ws).await?;
    let group = rec.group.unwrap_or_else(|| seq.to_string());

    let entry = JournalEntry {
        seq,
        ws: rec.ws.to_string(),
        actor: rec.actor.to_string(),
        surface: rec.surface.to_string(),
        tool: rec.tool.to_string(),
        kind: Kind::Do,
        class: rec.class,
        touched: Vec::new(), // nothing reversible was captured
        group,
        trace_id: rec.trace_id.to_string(),
        ts: rec.ts,
    };
    save_entry(store, rec.ws, &entry).await?;

    let mut stack = load_stack(store, rec.ws, rec.actor, rec.surface).await?;
    let pruned = stack.push_do(seq, rec.depth_cap.unwrap_or(DEFAULT_DEPTH_CAP));
    save_stack_pruning(store, rec.ws, &stack, &pruned).await?;

    Ok(seq)
}
