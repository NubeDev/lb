# Every retention sweep logs "cancelled transaction" in a workspace where no flow has run

**Area:** flows / jobs · **Status:** **fixed** (2026-11-13)

## Symptom

A node logs this every 2.5 minutes (every 30th reactor tick), for a workspace with no flow history:

```text
WARN lb_host::flows::retention_sweep: flow-run retention sweep failed ws=nube
  error=store backend error: The query was not executed due to a cancelled transaction
```

Harmless (there was nothing to trim) but permanent noise, and the message points nowhere near the
cause.

## Reproduce

`retain_runs(&Store::memory().await?, "ws-empty", 5)` on a fresh store returns the error above.
`lb_jobs::retain_terminal` does the same on a workspace with no jobs. On a real node only the flow-run
line shows, because the `job` table exists in any workspace that has run a job.

## Root cause

SurrealDB 3 raises `NotFoundError::Table` for a SELECT/DELETE against a table nothing has written.
`lb_store::check_absent_table_as_empty` turns that into "no rows", but only for a statement on its own.
Inside a `BEGIN … COMMIT` the error cancels every other statement first, the shim drops the
not-found slot, and the first leftover "cancelled transaction" is what the caller sees.

Both sweeps (`host/src/flows/retain_runs.rs`, `jobs/src/retain.rs`) open with a SELECT inside a
transaction. The SurrealDB 3 upgrade (#195) fixed this shape in `undo/src/prune.rs` and
`ingest/src/commit.rs` by declaring the tables first; these two were missed.

## Fix

Declare the tables at the start of the same transaction (`DEFINE TABLE IF NOT EXISTS`, idempotent),
as `undo/src/prune.rs` does: `flow_run` + `flow_step_output` in `retain_runs`, `job` in
`retain_terminal`.

The other transactions in the tree (`write_if_rev`, `write_tx`, `write_journaled`, `write_batch`,
`tags/add`, `undo/restore`, `capped_insert`) write first or read single records by id, which does not
need the table to exist, so they are not affected.

## Guard (regression tests)

- `host/tests/flows_retention_test.rs`: `retain_runs_on_a_workspace_with_no_flow_tables_is_a_no_op`
- `jobs/tests/retain_test.rs`: `retention_on_a_workspace_with_no_jobs_is_a_no_op`

Both fail on the old code with the exact log message above.

## Lessons

- Any transaction that reads before it writes must declare the tables it reads, because
  `lb_store`'s absent-table shim cannot reach inside one. Same lesson as `undo/src/prune.rs`.
