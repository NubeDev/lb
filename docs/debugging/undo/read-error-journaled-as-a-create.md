# A failed before-image read was journaled as a *create* (whose undo DELETES a live record)

- Area: undo
- Status: resolved
- First seen: 2026-07-15
- Resolved: 2026-07-15
- Session: ../../sessions/undo/undo-exposure-session.md
- Regression test: rust/crates/host/src/undo_capture/decide.rs
  (`read_error_is_not_undoable_never_absent`, `missing_snapshot_is_not_undoable`)

## Symptom

No runtime symptom — which is exactly why this is written down. The auto-capture path took the
before-image with:

```rust
read_versioned(store, ws, table, id).await.ok()   // Err → None
```

`.ok()` flattens a **read error** and a **genuine absence** into the same `None`. Absence is
meaningful here: it means "this call created the record", so the journaled undo is *delete it again*.
A transient store read failure therefore produced an undoable "create" entry for a record that
**already existed** — and undoing that step would DELETE live data the user never created.

Data loss, silent, only on the error path, and invisible to every green test: the happy path reads
fine, so nothing exercised the flattening.

## Cause

`Option` cannot carry the distinction the decision needs. Three input states (read-found,
read-absent, read-failed) were being squeezed into two, and the lost one was the dangerous one. The
capture logic then treated `None` as "absent" because that is what `None` means on the happy path.

## Fix

Make the third state representable and the decision total. `undo_capture/decide.rs` is now a pure
function over the whole outcome table:

```rust
Some(BeforeRead::Read(v)) => Decision::Undoable { .. }   // a successful read (present OR absent)
Some(BeforeRead::Failed) | None => Decision::NotUndoable // nobody observed the prior state
```

Only a **successful** read that finds nothing is a create. A read error journals the step
not-undoable: the honest answer, since no before-image was ever observed. Being pure, the whole table
is unit-testable without a store.

## Why it stayed hidden

`.ok()` reads as harmless idiom — "I don't care about the error". Here the error *was* the
information. Worth a second look anywhere `.ok()`/`unwrap_or_default()` sits on a read whose
**absence** carries semantic weight (undo, cache-fill, upsert-vs-create, exists-checks).

## Verification

Revert-checked: reintroducing the flattening (`Failed → Undoable { before: None }`) turns both
regression tests red; restoring the table turns them green. A test that never fails when the bug
returns is not a regression test.
