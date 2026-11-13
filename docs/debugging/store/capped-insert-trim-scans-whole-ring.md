# A busy node pegs 4–7 cores; every telemetry event scans the whole ring

**Area:** store · **Status:** **fixed** (2026-11-13, session `sessions/store/capped-trim-index-session.md`)

## Symptom

An embedded dev node (rubix-ai, debug build, `LB_TELEMETRY_SINK=both`) sat at 400–700% CPU for an
hour with nobody using it. The laptop ran hot. Gateway calls still answered, just slowly.

## Reproduce

1. Boot a node with the telemetry ring on (`LB_TELEMETRY_SINK=both` or `surreal`).
2. Let a few extensions poll the host (here: three sidecars, about 10 tool calls a second).
3. Wait until the ring fills: 1000 rows per source × ~20 sources ≈ 20k rows in `telemetry`.

From then on every tool call costs ~0.6 s of store CPU (debug build), several in parallel.

## Observed (not guessed)

- `sample` of the node: busy time is ~all SurrealDB — `TableScan`/`collect_table`,
  `Document::delete`, `check_where_condition`, `Expr::compute_binary`. The queries run on
  SurrealDB's router task, so the stack does not name the caller.
- Pausing each sidecar in turn (`SIGSTOP` for 6 s) barely moved CPU: no single caller was the cost,
  the cost was per call.
- The failing `flow-run retention sweep` (every 2.5 min) was not it: CPU did not dip after a failure.
- Store table sizes: everything under 3k rows except `telemetry` at 20,158. A `count()` on it took
  0.46 s against 0.01 s for the others; `INFO FOR TABLE telemetry` showed **no indexes**.
- One trim read (`SELECT VALUE seq … WHERE cap_key = 'store' ORDER BY seq DESC LIMIT 1000`) took
  0.49 s over the gateway.
- Isolated in a `mem://` test seeded to the same shape (20 keys × 1000 rows): **621 ms per
  `capped_insert`**, and `EXPLAIN` showed `TableScan` → `SortByKey`.

## Root cause

`lb_store::capped_insert` (`crates/store/src/capped.rs`) trims on every insert, by design, so the
ring never exceeds its cap. The trim was:

```sql
LET $keep = (SELECT VALUE seq FROM type::table($tb) WHERE cap_key = $key ORDER BY seq DESC LIMIT 1000);
DELETE FROM type::table($tb) WHERE cap_key = $key AND seq NOT IN $keep;
```

With no index on `cap_key`, both statements scan the whole ring — and a ring holds `cap` rows for
EVERY key, so its size is `cap × keys`, not `cap`. The DELETE then compares each row against a
1000-element array. The telemetry layer calls this once per event, so cost grows with ring size ×
call rate. Fine on a quiet node; several cores on a busy one. Every other `capped_insert` ring
(`insight_occ`, `entity_version`) had the same shape.

## Fix (2026-11-13)

1. **Index** `(cap_key, seq)` on every capped table — `crates/store/src/capped_index.rs`, ensured
   lazily per `(ws, table)` with `DEFINE INDEX IF NOT EXISTS` (the `lb_jobs::define_job_index`
   convention). One `OnceCell` per `(ws, table)` on the `Store`, so it is built once and concurrent
   first writers wait for that build instead of each starting their own. Best effort: a failed
   build is logged and the insert still lands (the trim is correct without the index, just slow).
2. **Cutoff, not keep-list**: the trim reads the first row past the newest `cap` and deletes
   everything at or older than it (`$cut[0] != NONE AND seq <= $cut[0]`). `seq` is unique (it is the
   record id), so the set deleted is identical; the per-row comparison against a 1000-item array is
   gone.

Measured once, locally, in a debug build against a `mem://` store seeded to the same shape
(20 keys × 1000 rows; the timing harness is not committed): **621 ms → 6.8 ms per insert**. The
first insert after upgrade pays a one-off **~384 ms** to build the index over the existing ring.
The trim read's plan is now `IndexScan capped_key_seq` (backward, limit 1 offset 1000); the DELETE
runs on SurrealDB's older executor, which `EXPLAIN` does not cover, so only the read's plan is
pinned by a test.

**Immediate relief** without a rebuild: `LB_TELEMETRY_SINK=stderr` (the in-product telemetry
console is empty while it is set).

## Guard (regression tests)

`crates/store/tests/capped_index_test.rs` (and `lowering_the_cap…` in `capped_test.rs`):

- `trim_reads_one_key_through_the_index` — the index exists after an insert, the trim's read is an
  `IndexScan` on it and never a `TableScan`, and each key is still trimmed to exactly its cap.
  Deterministic (asserts the plan, not a time). Fails on the old code.
- `non_identifier_table_is_refused` — the table name is interpolated into DDL, so a non-identifier
  is refused before it reaches SurrealDB.
- `trim_index_is_ensured_in_every_workspace` — workspace isolation: one store builds the index in
  each workspace's namespace, and each sees only its own rows.
- `existing_unindexed_ring_is_indexed_and_trimmed_on_first_insert` — the upgrade path.
- `concurrent_first_inserts_on_an_unindexed_ring_all_land` — 50 concurrent first writers.
- `insert_lands_and_trims_when_the_index_cannot_be_built` — a computed `cap_key` makes SurrealDB
  refuse the index; the row still lands and the ring is still trimmed.
- `lowering_the_cap_trims_to_the_new_cap_in_one_insert`.
- The existing six (FIFO, per-key vs global, concurrency, zero-cap clamp, body shape) pass unchanged.

Revert-checked: the index test and the identifier test fail on the old code; the workspace test
fails if the remembered key drops the workspace; the failed-build test fails if the build error is
returned instead of logged.

## Lessons

- A ring's row count is `cap × number of keys`. A per-key cap bounds each key, not the table.
- A trim that runs on every write is only as cheap as its WHERE clause. Anything written per event
  needs its filter indexed — the same lesson as `jobs/node-pegs-cpu-reactor-rescans-job-table.md`.
