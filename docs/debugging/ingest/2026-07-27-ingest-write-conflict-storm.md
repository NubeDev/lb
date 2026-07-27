# `ingest.write` transaction-conflict storm (dropped batches → raw-series gaps)

- Area: ingest
- Status: **fixed** (lb side: WS-A + WS-B; end-to-end proof is the rubix-ai WS-D e2e)
- First seen: 2026-07-27 (live node, ≥2 modbus networks pushing every 2s to one workspace)
- Session: ../../sessions/ingest-conflict-storm-session.md
- Scope: cross-repo, authored in `rubix-ai-extensions/docs/sessions/ingest-conflict-storm/scope.md`
- Regression test: `rust/crates/ingest/tests/ingest_conflict_storm_test.rs`

## Symptom

A live node logs a continuous storm of push failures:

```
[modbus] plant-b push failed: host callback failed: host returned HTTP 502:
extension error: store backend error: The query was not executed due to a failed
transaction. Failed to commit transaction due to a read or write conflict.
This transaction can be retried
```

SurrealDB says **"can be retried"** — a transient optimistic-concurrency abort, not corruption.
Nothing on the ingest path retried it, so each abort became a **dropped batch** of samples. Because
the modbus COV layer only emits on change, a dropped batch is a **permanent gap** in the raw series,
which then holes the 5-minute rollup grid the operator is trying to keep clean.

## Cause — three independent layers (this doc covers the two lb ones)

**L1 — the ingest transactional path never retried a retriable conflict.**
`store::Store::query_ws` (`store/src/open.rs`) runs a caller's `BEGIN…COMMIT` with no conflict
handling. `commit_batch` (`ingest/src/commit.rs`), `evict_raw` (`ingest/src/gc.rs`) and
`write_rollups`/`evict_rollups` (`ingest/src/rollup.rs`) all went through it. The retry primitive
**already existed** but only on the record-write path: `store/src/write_locked.rs`'s
`is_retryable_conflict()` / `MAX_CONFLICT_RETRIES` (and duplicated in `capped.rs` and `increment.rs`).

**L2 — concurrent inline drains collide structurally.**
`ingest.write` runs an inline bounded drain (`host/src/ingest/drain.rs`). Each drain `SELECT`s the
same oldest-256 staged rows (`commit.rs`, `ORDER BY seq,ts`) then commits a tx that `DELETE`s those
exact staging ids and `UPSERT`s the shared `series_latest` pointer. `commit_batch` reads its head
and commits in **two separate store round-trips**, so with ≥2 producers pushing, two drains (an
inline caller and the background reactor, or two callers) grab the same head between one's SELECT and
its COMMIT and one always loses the commit race. The 300s GC reactor deleting raw from `series` adds
a second, periodic collision surface (drain-vs-GC).

(L3 — modbus dropping must-deliver samples on a retriable failure — is fixed in the extension repo,
WS-C.)

## Fix

**WS-A — bounded retry-on-conflict for `series`-table mutations.**

1. Promoted the conflict primitive to one shared place: new `store/src/conflict.rs` with
   `pub(crate) is_retryable_conflict()`, `MAX_CONFLICT_RETRIES`, and a shared `conflict_backoff()`.
   `write_locked.rs`, `capped.rs`, and `increment.rs` now all call it — the match string
   (`"can be retried"` / `"read or write conflict"` / `"Invalid revision"`) lives in **exactly one
   function**, so the four retry loops can never drift.
2. New `Store::query_ws_retrying` (`store/src/open.rs`) — same signature as `query_ws`, loops the
   inner query, retries **only** when `is_retryable_conflict` matches (non-retriable errors return
   immediately), bounded by `MAX_CONFLICT_RETRIES` with the shared jittered-shape backoff.
3. Routed every `series`-family mutation through it: `commit_batch`'s `BEGIN…COMMIT`, `evict_raw`,
   `write_rollups`, `evict_rollups`. Reads (the drain `SELECT`) were left on plain `query_ws`.

   **Safety** (preserved in a comment on `query_ws_retrying` and `commit_batch`): retrying a whole
   `commit_batch` tx is safe because it is **atomic** (a conflict aborts and fully rolls back — no
   partial state) and **idempotent** (the UPSERT is keyed on `[series, producer, seq]` and it deletes
   exactly the staged rows it read), so a retry re-applies the batch exactly once — the same
   guarantee the exactly-once drain already relies on.

**WS-B — serialize drains per workspace.**

New `host/src/ingest/drain_lock.rs` — a process-wide `static` keyed-by-`ws` async lock (the same
idiom `write_locked`/`increment` use per record). `drain_at_most` (the one loop both the bounded
caller drain and the unbounded reactor drain share) now takes it **per `commit_batch`**, so a drain's
SELECT+commit is atomic w.r.t. other drains: no two drains ever grab the same staging head, and the
racing pair never forms.

**Per-batch, not per-pass, is deliberate.** Holding the lock across the whole pass would let the
reactor's unbounded (O(backlog)) drain block a concurrent inline caller for the entire backlog —
re-coupling caller latency to backlog, the exact regression
[`write-drains-whole-workspace-backlog.md`](write-drains-whole-workspace-backlog.md) removed. At
batch granularity the reactor releases between batches, so an inline caller waits at most one batch.
Commits still run inline on the caller's path (the write-then-read round-trip is untouched).

WS-A and WS-B are **complementary**: WS-B removes drain-vs-drain (the continuous storm); WS-A's retry
still absorbs drain-vs-GC (the periodic surface the drain lock does not cover — GC is not a drain).

## Regression test

`rust/crates/ingest/tests/ingest_conflict_storm_test.rs` (2 tests, real spawned `Store::memory()`
which is real SurrealDB `kv-mem` with the same optimistic MVCC — no fake store, Rule 9; several
worker threads so commits run genuinely in parallel):

- `serialized_drains_are_exactly_once` — **drain-vs-drain.** 6 producers stage one block; 6 drains
  race it, serialized per-ws (mirroring WS-B's `ws_drain_lock`, which lives in `lb_host` and cannot
  be imported into the ingest crate) with WS-A retry underneath. Asserts no conflict error and the
  committed series is exactly-once: every producer's full seq range, no gap, no duplicate.
- `serialized_drains_vs_gc_lose_no_samples` — **drain-vs-GC.** A settled historical block A is
  rolled-up-and-evicted by a GC looper while 6 serialized drains commit a NEW block B to the same
  `series` table; the blocks occupy disjoint stretches of the time axis (as in production — GC trims
  the old tail, writes hit the head). Asserts no conflict error from either side, B survives as raw
  exactly-once, and all of A is preserved in the rollup tier.

## Findings worth keeping (things measured, not assumed)

- **WS-A retry alone does NOT absorb N-way unserialized drain-vs-drain.** An early version ran 6
  *unserialized* drains and the 16-retry bound was exhausted (flakily). That is expected and is
  precisely why WS-B exists — the two are complementary, not alternatives. The test therefore
  serializes drains (modelling WS-B) and asserts WS-A retry handles the residual (drain-vs-GC, ≤2
  contenders).
- **The per-drain `committed` counter over-counts under unserialized racing drains** (a loser
  retries and re-runs the idempotent UPSERT the winner already applied), while the STORED rows stay
  exactly-once. Assert store state (`read`/`latest`), never the sum of per-drain counters.
- **`read_buckets` / `rollup_series` return nothing for sub-second timestamps.** A first test draft
  used `ts = seq` (1..350 ms) and GC evicted raw while writing zero rollups — a *test-induced* data
  loss with 10ms buckets, unrelated to conflicts. Retention/rollup tests must use a realistic ts
  cadence (the fixed test uses 1s). Recorded so it is not misread as a conflict bug; whether
  `read_buckets` should handle sub-second buckets is a separate question, out of scope here.

## Lessons

- **The retry primitive existed; it just was not on the transactional path.** The conflict matcher
  and bounded retry shipped for the flows run-store rev race (`write_locked.rs`) and were copied into
  `capped.rs`/`increment.rs`, but the ingest `BEGIN…COMMIT` never used any of them. Consolidating to
  one `conflict.rs` and adding one `query_ws_retrying` wrapper closed the gap without a new mechanism.
- **Serialize at the granularity of the conflict, not coarser.** The conflict unit is one
  `commit_batch` over one staging head; locking per batch kills the race without reintroducing the
  backlog-latency coupling a whole-pass lock would.
