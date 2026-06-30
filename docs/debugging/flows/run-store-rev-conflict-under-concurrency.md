# Run-store `rev` RMW races under concurrent same-record writes (`Invalid revision` / conflict)

- Area: flows (lb-store::write rev bump, driven by the flows run-store + lb-jobs)
- Status: resolved
- First seen: 2026-06-30
- Resolved: 2026-06-30
- Session: ../../sessions/flows/flow-plc-reliability-session.md
- Scope: ../../scope/flows/flow-plc-reliability-scope.md
- Precedent (same fix shape): ../observability/capped-insert-overgrows-cap-under-concurrency.md
- Regression tests:
  - rust/crates/store/tests/write_locked_test.rs::concurrent_same_record_writes_never_conflict
  - rust/crates/host/tests/flows_plc_reliability_test.rs::concurrent_same_run_id_never_conflicts_and_settles_once

## Symptom

A flows run never settled: the canvas banner showed `store backend error: … Invalid revision '174'
for type 'Value'`, and under a burst `Failed to commit transaction due to a read or write conflict.
This transaction can be retried`. Live, 8 concurrent `POST /flows/chain4/run` produced 6+ of these
errors (the constant run id — see `frozen-gw-now-collides-run-ids.md` — guaranteed they all hit the
same records).

## Reproduce

`cargo test -p lb-host --test flows_plc_reliability_test
concurrent_same_run_id_never_conflicts_and_settles_once` on the pre-fix code: 8 concurrent
`flows.run` of the **same** run id surface
`Extension("store backend error: … read or write conflict. This transaction can be retried")`.
(The durable `kv-surrealkv` engine on the live node raises it far more readily than `mem://`, but the
multi-thread `mem://` test reproduces it too.)

## Root cause

`lb_store::write` derives the new `rev` server-side (`(rev ?? 0) + 1`) so a *single* write is atomic.
But two writers targeting the **same** `table:id` each open an optimistic, snapshot-isolated
transaction over the same prior `rev` snapshot. Under the durable engine one commits and the other
aborts with a retryable conflict — or a later read deserializes a half-applied rev as
`Invalid revision`. The same `flow_run` / `flow_step:*` / `job:{run_id}` rows were written by
concurrent seeds + drives (made certain by the shared run id), so the latent race became a wall of
errors. The bare `write` had no serialization and no retry — exactly the shape the capped ring hit.

## Fix

Port the proven `capped_insert` discipline into the write primitive as a **store-level** variant
(preferred placement — every same-record writer benefits, not just the flows caller):
`lb_store::write_locked` (`crates/store/src/write_locked.rs`) = an in-process per-`(ws,table,id)`
async lock (removes the interleaving that defeats snapshot isolation) **plus** a bounded
retry-on-retryable-conflict (`MAX_CONFLICT_RETRIES`, escalating sub-ms backoff so a burst
desynchronizes rather than livelocks). Same observable result as `write` (same monotonic rev bump,
same taint).

Callers on the run hot path switched to it via a one-line alias (`write_locked as write`):
`crates/host/src/flows/run_store.rs`, `crates/jobs/src/{create,update}.rs` (the `job:{run_id}` record
is written by both the seed and `complete`). Plus `create_run` now does a **create-if-absent seed**
under a per-`(ws,run_id)` lock, so a racing second `start` no-ops instead of re-seeding `pending`
over an in-flight run.

Considered and rejected: localizing the lock/retry inside `run_store.rs` only (leaves every other
same-record writer — jobs, future callers — exposed; the store-level primitive is the correctness
boundary). Retry-only without the lock (the same-record interleaving still corrupts the rev — no
conflict is raised for every shape, mirroring the capped precedent).

## Verification

`write_locked_test::concurrent_same_record_writes_never_conflict` (16 concurrent same-record writes →
no error, coherent `rev == 16`) and `concurrent_same_run_id_never_conflicts_and_settles_once` (8
concurrent same-id runs → no conflict, settles `success` once) both green. Live: 8 concurrent runs,
node log clean of `Invalid revision` / `conflict`.

## Prevention

The two concurrency regressions are the standing guards; they fail-before / pass-after. Because the
fix is in the `lb-store` primitive, any future caller that can write a record concurrently can adopt
`write_locked` and inherit the guarantee.
