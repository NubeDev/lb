//! Journal a reversible single-record change that was **already applied** by the tool itself —
//! the auto-capture-on-dispatch path (`docs/scope/undo/undo-scope.md` "Intent": capture at the
//! dispatch/store seam).
//!
//! [`record_change`](crate::record_change) is for callers that hand the journal the new value and
//! let it apply + journal atomically. At the host **dispatch seam** the tool has *already* run and
//! written the record, so we cannot re-apply — we snapshot the before-image *before* the call, let
//! the call run, then call this to read the produced after-image/`rev` and write the journal entry.
//!
//! The honest limit (v1 floor): this is correct only for a self-contained **single-record** upsert
//! whose `(table, id)` the dispatch seam can name. A tool whose durable footprint the seam cannot
//! see (raw `query_ws`, multi-record, derived state) is **not** captured here — it is marked
//! not-undoable via [`record_irreversible`](crate::record_irreversible) instead of partially
//! captured (scope "non-generic capture": never a partial raw restore that leaves invariants
//! broken).
//!
//! Atomicity note: unlike `record_change`, the change and the journal entry do NOT share one
//! transaction here (the change already committed inside the tool). The journal is therefore a
//! best-effort *after* record of a committed change; a crash between the two leaves a committed
//! change with no undo entry (it is simply not-undoable), never an orphan entry for a change that
//! did not land. That is the correct failure direction for dispatch capture.

use lb_store::{read_versioned, Store};
use serde_json::Value;

use crate::error::UndoError;
use crate::model::{Class, JournalEntry, Kind, TouchedRecord, DEFAULT_DEPTH_CAP};
use crate::persist::{load_stack, next_seq, save_entry};
use crate::prune::save_stack_pruning;

/// What a caller hands us to journal an already-applied reversible single-record change.
pub struct RecordCaptured<'a> {
    pub ws: &'a str,
    pub actor: &'a str,
    pub surface: &'a str,
    pub tool: &'a str,
    pub trace_id: &'a str,
    pub ts: u64,
    /// The record the tool touched.
    pub table: &'a str,
    pub id: &'a str,
    /// The record's state captured **before** the tool ran (`None` = it was absent → a create).
    pub before: Option<Value>,
    /// The record's `rev` before the tool ran (`ABSENT_REV`/0 when `before` is `None`).
    pub before_rev: u64,
    /// Group id for a multi-step action (a job/batch). `None` = a standalone step (group = its own
    /// seq). Threaded through so grouped undo can reverse a whole group all-or-nothing later.
    pub group: Option<String>,
    /// Bounded stack depth: steps that fall past this are pruned (event + live companion deleted)
    /// in the same transaction as the cursor push. `None` = [`DEFAULT_DEPTH_CAP`].
    pub depth_cap: Option<usize>,
}

/// Journal a `Do` entry for an already-applied reversible change, reading its after-image + `rev`
/// from the store, and push the step onto the (ws, actor, surface) undo stack. Returns the seq.
pub async fn record_captured(store: &Store, rec: RecordCaptured<'_>) -> Result<u64, UndoError> {
    // The after-image + produced rev (read post-commit). Absence → a delete (after: None, rev 0).
    let after = read_versioned(store, rec.ws, rec.table, rec.id).await?;

    let seq = next_seq(store, rec.ws).await?;
    let group = rec.group.unwrap_or_else(|| seq.to_string());

    let entry = JournalEntry {
        seq,
        ws: rec.ws.to_string(),
        actor: rec.actor.to_string(),
        surface: rec.surface.to_string(),
        tool: rec.tool.to_string(),
        kind: Kind::Do,
        class: Class::Reversible,
        touched: vec![TouchedRecord {
            table: rec.table.to_string(),
            id: rec.id.to_string(),
            before: rec.before,
            after: after.value,
            expected_after_rev: after.rev,
            expected_before_rev: rec.before_rev,
        }],
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
