# The retention sweep failed on every cycle where a table did not exist yet

**Status: fixed.** 2026-09-28. Found from an embedding node (rubix-ai), whose log carried

```
WARN lb_host::flows::retention_sweep: flow-run retention sweep failed ws=nube
     error=store backend error: The query was not executed due to a cancelled transaction
```

every 2½ minutes for the life of the node.

## Cause

Both retention verbs — `flows::retain_runs` and `lb_jobs::retain_terminal` — ran as one
`BEGIN … RETURN count(…); COMMIT` transaction. **Under SurrealDB 3, reading or deleting a table
that does not exist yet fails the enclosing transaction.** Outside a transaction the same
statements are fine on a missing table (measured statement by statement).

So the sweep failed, forever, on:

| Workspace state | Message |
|---|---|
| no flow tables at all | `…due to a cancelled transaction` (the live one) |
| finished runs, no `flow_step_output` rows | `…due to a failed transaction` |
| never wrote a job | the same, in the jobs half |

Nothing was trimmed in those workspaces, and the warning read as noise because it never stopped.

## What it was NOT

Contention. Five sweeps on a real surrealkv store under concurrent run+step writers all
succeeded (kept as a regression test). The first hypothesis — optimistic-concurrency conflicts
cancelling the transaction — was wrong; the store engine and the volume were irrelevant too
(it failed purging 2 rows on `mem://`).

**Why CI was green:** the existing tests always seeded step rows, so the step table always
existed. The one shape that fails — a workspace that has not written the table yet — is the
shape every fresh workspace starts in.

## Fix

Two round-trips instead of a transaction:

1. read the purge set (the `capped.rs` `LET $keep … NOT IN $keep` idiom) into Rust; empty → 0;
2. delete **step rows first, then runs**, keyed on that set.

The transaction only ever protected one thing — a run deleted while its step rows survive,
orphaned for good. Steps-first makes that partial failure harmless: if the run delete fails, the
runs are still there and the next sweep computes the same set. Every step is idempotent. The
count is the set's length, so there is no `RETURN` slot to decode — a top-level `RETURN` outside
a transaction does not come back the way the in-transaction one did (5 statements, 4 empty slots).
The jobs verb, one table, gets the same two-trip shape.

## Regression

`host/tests/flows_retention_test.rs`: a workspace with no flow tables returns 0; runs without a
step table are trimmed; concurrent writes on surrealkv leave no orphaned step row.
`jobs/tests/retain_test.rs`: a workspace with no job table returns 0. The first two, and the jobs
one, fail on the transactional code (revert-checked) and pass on the fix.

## Still to check

Other `BEGIN TRANSACTION` queries may share the trap on a fresh workspace: `undo/src/prune.rs`,
`undo/src/restore.rs`, `store/src/capped.rs`, `store/src/write_batch.rs`,
`store/src/write_journaled.rs`, `store/src/write_tx.rs`, `ingest/src/commit.rs`,
`tags/src/add.rs`. Not changed here. A read-then-write inside `BEGIN` on a table that may not
exist yet is the shape to look for.
