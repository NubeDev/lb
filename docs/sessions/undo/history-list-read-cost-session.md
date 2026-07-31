# Session — history.list read cost: kill the N+1, ship the gate flags

**Scope:** `docs/scope/undo/history-list-read-cost-scope.md` · **Date:** 2026-07-31
**Outcome:** built + tested + mutation-checked. One additive result field pair; no verb rename, no
cap change, no consumer break.

## The finding this closes

`history.list` was the slowest read verb on a dashboard open and it grew with every edit. Measured
live on `demo-operations` with a 99-entry journal: **432–437 ms**, 9.5 KB, every run — against
8–13 ms for `dashboard.get` and `datasource.list`. The derived unit cost of one store `load_entry` is
~4.3 ms; 99 × 4.3 ≈ the whole 432 ms.

Textbook N+1: `lb_undo::list` loaded the stack and then **serially awaited one store read per seq**.
The one consumer on the hot path (rubix-ai's dashboard `select()`) reduced all 99 items to two
booleans, and React StrictMode fired it twice in dev — ~900 ms of store work per board open to answer
"may the two toolbar buttons enable".

## What was built

`crates/undo/src/history.rs`:

- `load_entries()` — a private fn beside `list`, not a util. Loads the seqs with
  `futures::future::join_all` in chunks of **32**, returning results **positionally** so the caller
  reassembles in seq order, never completion order. A real store error propagates; only a *missing*
  entry (`Ok(None)`) is skipped, exactly as the serial loop did.
- `HistoryList { items, can_undo, can_redo }` replaces the bare `Vec<HistoryItem>`. `items` is
  unchanged in shape and order. `can_undo` = some entry on the undo side is actually **reversible**
  (not "the undo side is non-empty" — a stack of purely non-undoable steps must gate the button off);
  `can_redo` = anything has been undone.
- A relaxed-atomic in-flight gauge (`peak_in_flight` / `reset_in_flight_peak`) — two atomic ops next
  to a store round-trip, and the only way to assert the fan-out bound without mocking the store.

`crates/host/src/undo/history.rs` returns the `HistoryList` whole (same gates: `mcp:history.list:call`
plus `undo.any` for another actor's stack — the flags are derived only from entries the caller was
already authorized to list). `crates/host/src/tool_call.rs` spreads it onto the verb result as
`{items, can_undo, can_redo}`, and the catalog description gains a sentence.

## Decisions taken where the scope was silent

- **The gauge is a plain `pub fn`, not `#[cfg(test)]`.** The crate's tests are integration tests
  (`tests/*.rs`), so `cfg(test)` instrumentation is invisible to them — the same reasoning as
  `SqliteSource::built_slots()` in the federation scope. Its doc comment says it is a test seam and
  that nothing in the product path reads it.
- **The bounded-concurrency test lives in its own file.** Cargo compiles each `tests/*.rs` into its
  own binary; the gauge is process-global, so sharing a binary with other tests that call `list`
  would let their concurrency land in this test's reading — a flake that would look exactly like a
  real bound violation.
- **The chunk bound is duplicated in the test rather than imported.** The test asserts the
  *contract* ("never more than 32 concurrent entry reads"); importing the constant would let a bad
  edit move the goalposts and the assertion silently along with it.

## Tests (real in-memory store, real journal writes — no mocks)

`crates/undo/tests/history_list_test.rs`:

1. `items_keep_their_order_and_content` — the order golden: undo side newest-first, then redo side
   newest-first, captured from the pre-change serial behavior.
2. `flags_match_the_fold_consumers_do` — empty / mixed / all-undone / **non-undoable-only**, plus a
   loop asserting each flag equals the fold every consumer does today.
3. `missing_entry_is_skipped` — a pruned journal row is skipped, not an error; flags stay honest.
4. `hundred_entry_list_is_not_a_staircase` — 100 entries; wall asserted under 1/3 of a serial
   baseline measured on the same rig in the same process.

`crates/undo/tests/history_list_concurrency_test.rs` — `entry_loads_stay_bounded`: peak in-flight
≤ 32 **and** > 1 (the second half is what makes it a real fix, not just a safe one).

**Mutation checks run (not just written):**

| Mutation | Result |
|---|---|
| `can_undo = !stack.undoable.is_empty()` | `flags_match_the_fold_consumers_do` FAILED |
| `LOAD_CHUNK = 1000` | FAILED: *"list held 100 concurrent entry reads on a 100-entry stack"* |
| entry loads made serial again (the original bug) | FAILED: *"list held only 1 entry read at a time"* |

Restored, `cargo test -p lb-undo` is green: 1 + 4 + 10 passed, 0 failed.

## Call-site fallout (the `Vec` → struct change)

Eight test call sites across `lb-host` and `lb-role-gateway` read `.items` now. No production caller
outside the two files above touched the value — which is what made the additive shape safe.

## Notes for whoever reads this next

- The non-goals hold: no `limit`/pagination (a limited read cannot answer `can_undo` honestly), no
  journal storage change, no change to `undo`/`redo`/`history.compensations` semantics.
- Downstream: rubix-ai's `dashboard-load-tail-scope` §C consumes the flags with a fold fallback, so
  it is correct against an un-bumped pin. The tag + pin bump is a release step, not done here.
