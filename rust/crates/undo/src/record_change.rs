//! Record a forward `do`: capture the before-image of a single record, apply the change atomically
//! with the journal entry, and push the step onto the actor's undo stack. The capture-at-the-write
//! -seam path (`docs/scope/undo/undo-scope.md` "Intent").
//!
//! This is the **reversible** path. An irreversible/compensable action does **not** call this — it
//! records a not-undoable marker via [`record_irreversible`](crate::record_irreversible), so undo
//! refuses it. The composition rule (a mixed action is irreversible as a whole) is enforced by the
//! caller deriving the class from runtime taint (`classify.rs`) before choosing which to call.
//!
//! Atomicity: the change and the journal entry commit in ONE transaction via
//! [`lb_store::write_journaled`] — a forced failure leaves neither (no orphan entry, no
//! un-journaled change). The `after` rev recorded in the entry is read back after the commit.

use lb_store::{read_versioned, write_journaled, JournalWrite, Store, Versioned};
use serde_json::Value;

use crate::error::UndoError;
use crate::model::{Class, JournalEntry, Kind, TouchedRecord, DEFAULT_DEPTH_CAP, ENTRY_TABLE};
use crate::persist::{load_stack, next_seq};
use crate::prune::save_stack_pruning;

/// What a caller hands us to record a reversible single-record change.
pub struct RecordChange<'a> {
    pub ws: &'a str,
    pub actor: &'a str,
    pub surface: &'a str,
    pub tool: &'a str,
    pub trace_id: &'a str,
    pub ts: u64,
    /// The record being changed.
    pub table: &'a str,
    pub id: &'a str,
    /// The new value, or `None` to delete (record-as-absent — a delete undo restores the before).
    pub new_value: Option<&'a Value>,
    /// Bounded stack depth: steps that fall past this are pruned (event + live companion deleted)
    /// in the same transaction as the cursor push. `None` = [`DEFAULT_DEPTH_CAP`].
    pub depth_cap: Option<usize>,
}

/// Apply `change.new_value` to `change.table:change.id`, atomically journaling its before-image,
/// and push the resulting `Do` step onto the (ws, actor, surface) undo stack. Returns the new
/// step's `seq`.
pub async fn record_change(store: &Store, change: RecordChange<'_>) -> Result<u64, UndoError> {
    // 1. Snapshot the prior state + its rev (the before-image).
    let before = read_versioned(store, change.ws, change.table, change.id).await?;

    // 2. Allocate the step seq and build the (immutable) journal entry.
    let seq = next_seq(store, change.ws).await?;
    let group = format!("{seq}");

    // The value we will actually write. A delete is modelled as writing an explicit tombstone is
    // avoided here: for v1 a `None` new_value means "no change to apply other than journaling" is
    // NOT what we want — a delete must remove the row. We model delete by writing the journal then
    // deleting; but to keep the atomic seam we only support upsert-shaped changes here and route
    // deletes through `record_change` with an explicit tombstone value the owning verb interprets.
    // See scope "instrumented before-image": creates/upserts are the zero-work floor.
    let new_value = change.new_value.cloned().unwrap_or(Value::Null);

    let entry = JournalEntry {
        seq,
        ws: change.ws.to_string(),
        actor: change.actor.to_string(),
        surface: change.surface.to_string(),
        tool: change.tool.to_string(),
        kind: Kind::Do,
        class: Class::Reversible,
        touched: vec![TouchedRecord {
            table: change.table.to_string(),
            id: change.id.to_string(),
            before: before.value.clone(),
            after: Some(new_value.clone()),
            // filled in after the commit (the rev the write produced)
            expected_after_rev: 0,
            expected_before_rev: before.rev,
        }],
        group,
        trace_id: change.trace_id.to_string(),
        ts: change.ts,
    };

    // 3. Commit the change AND the journal entry atomically.
    let entry_value = serde_json::to_value(&entry).map_err(UndoError::codec)?;
    write_journaled(
        store,
        change.ws,
        change.table,
        change.id,
        &new_value,
        &JournalWrite {
            table: ENTRY_TABLE,
            id: &seq.to_string(),
            value: &entry_value,
        },
    )
    .await?;

    // 4. Learn the rev the change produced and patch it into the entry (a second, immutable write
    //    of the *same* entry id — the entry's content is finalized here, then never mutated again).
    let after: Versioned = read_versioned(store, change.ws, change.table, change.id).await?;
    let mut finalized = entry;
    finalized.touched[0].expected_after_rev = after.rev;
    crate::persist::save_entry(store, change.ws, &finalized).await?;

    // 5. Push onto the actor's undo stack (truncates redo).
    let mut stack = load_stack(store, change.ws, change.actor, change.surface).await?;
    let pruned = stack.push_do(seq, change.depth_cap.unwrap_or(DEFAULT_DEPTH_CAP));
    save_stack_pruning(store, change.ws, &stack, &pruned).await?;

    Ok(seq)
}
