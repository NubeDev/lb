# Session — federation read concurrency: K read slots per sqlite source

**Scope:** `docs/scope/datasources/federation-read-concurrency-scope.md` · **Date:** 2026-07-31
**Outcome:** built + tested + mutation-checked. Zero API surface change.

## The finding this closes

A dashboard open fans N panel queries at one datasource. The transport was already concurrent
(`native-call-concurrency`) and `viz.query_batch` already fanned out under a 16-permit semaphore —
and then every same-source read funnelled into **one connection**, so the fan-out collapsed back
into a serial staircase. Measured live on the seeded `demo-buildings` sqlite (5.8 M-row
`point_reading`, warm pool, result cache off): 4 identical scans completed at 228 / 444 / 654 /
865 ms — a pure staircase — and a 10-panel `viz.query_batch` measured 3,491–3,508 ms across 5 runs
while 8 parallel `SELECT 1` measured 15 ms wall. The transport was never the ceiling.

The cause: the upstream `datafusion-table-providers` `SqliteConnectionPool` is a **single**
tokio-rusqlite connection, and every cached `TableProvider` is bound to the pool it was built
against.

## What was built

`crates/federation/src/source/sqlite.rs` — `SqliteSource` now holds `slots: Vec<Mutex<Option<
Arc<ReadSlot>>>>` where each `ReadSlot` is its own `SqliteConnectionPool` (its own OS connection)
**plus its own per-table provider cache**. The pairing is the invariant: a provider holds the pool it
was built from, so sharing one across slots would funnel the reads back onto one connection.

- `connect()` builds slot 0 eagerly — so a bad file still fails *at connect*, with the same message —
  and leaves slots 1..K `None`. A source that only ever sees serial traffic never opens K connections.
- `table_provider()` and `query_direct()` both take `slot = next_slot.fetch_add(1) % K` and resolve
  against that slot. One atomic add is the entire scheduling policy.
- The slot-creation `Mutex` is **never held across an `.await`** (a `std::sync::MutexGuard` is not
  `Send`); a lost race on a cold slot builds one redundant pool and keeps the winner.
- `K = 4`, a constant, deliberately not env-tunable (the child runs 4 worker threads).
- The **write path is untouched** — `apply_ddl`/`write_rows`/`delete_rows` and the direct catalog
  reads keep their own short-lived `rusqlite` opens. sqlite has one writer by design.

`crates/federation/src/source/mod.rs` — the **read-concurrency contract** is stated on the `Source`
trait, not on a kind check (rule 10): concurrent reads on one source must proceed concurrently,
bounded by the source; N concurrent reads must cost ≈ the slowest, not the sum. The postgres source
already rides a real multi-connection pool and is conformant as written.

## Decisions taken where the scope was silent

- **`query_direct` rides the round-robin too.** The scope named `query()`/`table_provider`; pinning
  direct reads to slot 0 would have serialized them against each other and against the provider path
  — the same bug in a second doorway.
- **`built_slots()` is a plain `pub fn`, not `#[cfg(test)]`.** The integration tests compose the
  module by `#[path]` rather than linking the binary, so a test-only item is invisible to them. It is
  documented as the observable seam; the binary never calls it.
- **The `READ_SLOTS` re-export carries `#[allow(unused_imports)]`.** The binary never names K — only
  the tests (and their mutation check) do. The comment says so, so the allow does not read as an
  oversight.

## Tests (real engine, real files — no mocks)

`crates/federation/tests/read_concurrency_test.rs`:

1. `concurrent_reads_do_not_serialize` — the headline. Seeds a real 120 k-row sqlite file, measures a
   serial baseline in the same process, then runs 4 concurrent scans and asserts the wall is under
   2× the unit cost (serial measures ≈4×, so the pass/fail gap is wide and machine-relative).
2. `concurrent_reads_return_their_own_rows` — 4 concurrent predicates over distinct buckets each get
   exactly their own rows; catches a shared or miskeyed provider cache.
3. `connect_builds_exactly_one_slot` — laziness; the cost half of the trade.

Plus `read_concurrency_live_bench.rs` (`#[ignore]`, `LB_BENCH_SQLITE=…`) — the measurement rig
against the real seeded database, so the scope's before/after numbers stay reproducible.

**Mutation check run (not just written):** `READ_SLOTS = 1` →

```
test connect_builds_exactly_one_slot ... ok
test concurrent_reads_do_not_serialize ... FAILED
  4 concurrent scans took 8.470375ms, which is not meaningfully better than serializing them
  (unit=2.138351ms, ceiling=4.276702ms). Reads are funnelling onto one connection — the
  read-slot round-robin is not working.
```

Restored to 4, the whole `federation` test binary is green: **34 passed, 0 failed** (196 s — the
suite seeds and scans real files).

## Notes for whoever reads this next

- The scope's non-goal holds: this removes the ×N serialization multiplier, **not** the ~240 ms unit
  cost. Indexes/decimation/pushdown remain a separate concern, and the result cache above is
  unchanged and still the only thing that makes an identical re-open ~ms.
- Downstream: rubix-ai's `dashboard-load-tail-scope` needs **no UI change** for this half — only the
  `lb-node` tag bump (WORKFLOW-LB §4), which is a release step and is deliberately not done here.
