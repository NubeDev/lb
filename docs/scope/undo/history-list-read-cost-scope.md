# history.list read cost — kill the N+1, ship the gate flags

**Topic:** `undo` · **Name:** `history-list-read-cost` · **Status:** **built** (2026-07-31) — `lb_undo::list` loads entries concurrently in 32-chunks and returns `{items, can_undo, can_redo}`; the host verb carries the flags through additively. Tests + mutation checks in `crates/undo/tests/history_list_test.rs` + `history_list_concurrency_test.rs`. Session: `docs/sessions/undo/history-list-read-cost-session.md`.

`history.list` on a long-lived surface is the slowest read verb on a dashboard open, and it grows
with every edit. Measured live (2026-07-31, `demo-operations`, 99 journal entries):

| Verb | Measured | Notes |
|---|---|---|
| `history.list` | **432–437 ms**, 9.5 KB, every run | vs 8–13 ms for `dashboard.get`/`datasource.list` |
| store `load_entry`, derived unit | ~4.3 ms | 99 × 4.3 ≈ the whole 432 ms |

The cause is a textbook N+1 in `lb_undo::list` (`crates/undo/src/history.rs`): it loads the stack,
then **serially awaits one `load_entry` store read per seq** — 99 sequential round-trips. The one
consumer on the hot path (rubix-ai's dashboard `select()`) then reduces all 99 items to two
booleans (`canUndo`/`canRedo`), and React StrictMode fires it twice in dev — ~900 ms of store work
per board open to answer "may the two toolbar buttons enable".

## Goals

- **`history.list` on a 100-entry surface answers in ~the cost of its slowest single store read**
  (tens of ms), not the sum. Acceptance: the 99-entry surface above drops from ~432 ms to <50 ms.
- **The gate question gets a flat answer.** The result carries top-level `can_undo` / `can_redo`
  booleans (computed server-side over the full stack, exact), so a caller that only gates buttons
  reads two fields instead of folding N items.
- **Wire-compatible.** `items` keeps its exact shape and order; the two flags are additive. No verb
  rename, no cap change, no consumer breaks.

## Non-goals

- No pagination/`limit` arg. A limited read cannot answer `can_undo` honestly (an undoable entry
  can sit below any cutoff), which would push consumers back into the N-item fold this scope
  removes — and with the N+1 gone, the full list is cheap enough that paging solves nothing today.
- No journal storage change, no new store primitive, no eviction/compaction of long stacks (a
  100-entry journal read at ~30 ms is not a storage problem).
- No change to `undo`/`redo`/`history.compensations` semantics.

## Intent / approach

Two moves in `crates/undo/src/history.rs::list`, one in the verb result:

1. **Concurrent entry loads.** Collect the seqs from both sides of the stack, load them with
   `futures::future::join_all` chunked at 32 in-flight (bounded — a 500-entry stack must not open
   500 simultaneous store reads), then assemble items in the exact current order (undoable
   newest-first, then redoable newest-first). Missing entries are skipped exactly as today. No
   store API change — the same `load_entry`, called concurrently instead of one-by-one.
2. **Server-computed flags.** While assembling: `can_undo = any(undo-side item with
   class.is_undoable())`, `can_redo = !stack.redoable.is_empty()` — the same predicates the UI
   folds today, computed where the data already is.
3. **Result shape:** `{ items: [...], can_undo: bool, can_redo: bool }` — `items` byte-identical to
   today's, the flags additive at the top level (carry-don't-strip friendly for every existing
   consumer, which all read only `items`).

### Rejected alternatives

- **A batch/range store read for journal entries:** faster in principle, but it adds a store
  primitive for one caller when bounded `join_all` over the existing read already collapses the
  latency to ~one round-trip. If a future profile shows the 32-chunk still binding, that is the
  next scope, with this one's measurement as its baseline.
- **A separate cheap `history.flags` verb:** a second verb + cap + catalog row to avoid two bytes
  on an existing result. The flags belong on the read they summarize.
- **Client-side caching of the gates:** the journal is backend-authoritative by design
  (undo-exposure scope — the client holds NO local stack); guessing gates client-side reintroduces
  exactly the drift that rule exists to prevent.

## How it fits the core

- **Capabilities & deny path:** unchanged — same `mcp:history.list:call` gate, same `undo.any`
  cross-actor check, flags computed only over entries the caller was already authorized to list.
- **Workspace isolation:** unchanged — same ws-scoped stack + entry reads, now concurrent.
- **MCP surface:** one additive result field pair on an existing verb. Catalog description gains a
  sentence; no schema break.
- **Rule 10:** nothing kind- or consumer-specific; the flags are derived from the same class
  predicates any caller could apply.
- **One responsibility per file:** stays in `history.rs`; the chunked-concurrent load is a private
  fn beside `list`, not a util.

## Example flow

1. A dashboard `select()` fires `history.list {surface}` alongside `dashboard.get`.
2. The 99 entry reads go out 32 at a time; the verb returns in ~30 ms with
   `{items, can_undo: true, can_redo: false}`.
3. The UI sets its two gates from the flags. The Version-history-style dialogs keep reading
   `items` exactly as before.

## Testing plan

- **Order-preservation:** a stack with interleaved undoable/redoable entries returns `items` in
  byte-identical order/content to the current implementation (golden captured from the serial code
  before the change — that golden IS the regression pin).
- **Flags:** `can_undo`/`can_redo` match the fold every existing consumer does, across empty stack,
  all-undone, mixed, and non-undoable-class-only stacks. **Mutation check:** flip either predicate
  and watch it go red.
- **Bounded concurrency:** a 100-entry stack never exceeds 32 concurrent `load_entry` calls
  (instrumented store spy in the test — counting, not mocking the store).
- **Perf pin (real store):** 100 seeded entries; `list` wall-time asserted < 1/3 of the measured
  serial baseline on the same rig. Deliberately loose (CI noise) while still impossible for the
  serial code to pass.
- **Missing-entry skip:** delete one journal entry mid-stack; the item is skipped, flags stay
  correct — same as today.

## Risks & hard problems

- **Store contention:** 32 concurrent reads on stores tuned for serial access. The chunk bound is
  the mitigation; the perf pin catches a store where concurrent reads are pathological.
- **Golden brittleness:** the order golden must be captured from the pre-change code, not written
  by hand — a hand-written golden would pin the author's assumption instead of the behavior.

## Open questions

None — resolved by decision in this scope: bounded `join_all` (chunk 32) over the existing
`load_entry`, no new store primitive; flags on the existing verb result, no new verb, no `limit`
arg.

## Related

- `undo-scope.md` / `undo-exposure-scope.md` — the journal mechanism + the "backend-authoritative,
  no client stack" rule the flags serve.
- Downstream consumer: rubix-ai `docs/scope/frontend/dashboard/dashboard-load-tail-scope.md`
  (adopts the flags, stops awaiting the read on the select path).
