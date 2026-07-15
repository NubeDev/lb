# The undo depth cap trimmed the cursor but never deleted the events (unbounded journal)

- Area: undo
- Status: resolved
- First seen: 2026-07-15
- Resolved: 2026-07-15
- Session: ../../sessions/undo/undo-exposure-session.md
- Regression test: rust/crates/undo/tests/undo_test.rs
  (`pushing_past_the_depth_cap_deletes_the_fallen_off_events`)

## Symptom

Invisible from every read path, which is the whole problem. `StackState::push_do` enforced the
bounded depth by dropping the oldest seq off the cursor:

```rust
while self.undoable.len() > depth_cap {
    self.undoable.remove(0);   // dropped from the cursor…
}
```

…and that was all. The `undo:{seq}` event and its `undo_live:{seq}` companion **stayed in the store
forever**, now referenced by nothing. `history.list` looked correct (it only walks the cursor), the
stack was correctly capped, and the journal grew without bound for the life of the workspace. The
scope says "bounded depth, prune on push, no background sweeper" — the cursor was bounded; the
journal was not.

Same family as the ingest/series retention gaps: a plane with no reclaim path, where every read
looks right because readers only ever see the reachable set.

## Cause

`push_do` computed which seqs fell off and then **threw that information away** (`remove(0)`'s return
value was dropped). Nothing downstream could delete what nothing upstream reported.

## Fix

Make the caller unable to ignore the prune. `push_do` is now `#[must_use]` and **returns** the
fallen-off seqs; new `crates/undo/src/prune.rs::save_stack_pruning` commits the trimmed cursor and
the deletions in ONE transaction:

```
BEGIN TRANSACTION;
  UPSERT undo_stack:{id} CONTENT { data: $sdata, rev: … + 1 } RETURN NONE;
  DELETE undo:{seq}; DELETE undo_live:{seq};   -- per pruned seq
COMMIT TRANSACTION;
```

One transaction is load-bearing: there is no window where the cursor and the events disagree (a crash
mid-prune can't leave a cursor pointing at a deleted event, or an event the cursor can't reach). An
empty prune short-circuits to a plain `save_stack`, so the common path pays no transaction cost.

`depth_cap: Option<usize>` was added to `RecordCaptured`/`RecordChange`/`RecordIrreversible`
(`None` = `DEFAULT_DEPTH_CAP`, so no caller's behaviour changed) purely so the floor is provable in
**3 writes instead of 101**. A test too expensive to write is a test that doesn't get written.

## Why a test almost missed it

The obvious assertion — "the stack has 2 entries after 3 writes" — passes with the bug fully present:
the *cursor* was always correctly capped. The regression test therefore reads the `undo` table
**directly** (`read(&store, ws, "undo", &evicted_seq)`) and asserts the row is gone. Asserting
through `list` would have proven nothing, because `list` cannot see an orphan.

## Verification

Revert-checked: passing `&[]` instead of `&pruned` to `save_stack_pruning` (i.e. trim the cursor,
orphan the events) turns the test red with "the pruned journal event must be deleted, not orphaned
(unbounded growth)". Restoring it turns it green.
