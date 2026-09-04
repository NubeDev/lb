# Ingest scope — remove staging: one write per sample

Status: **IMPLEMENTED, unreleased.** Branch `feat/remove-ingest-staging`, cut from
`feat/surrealdb-3-upgrade`. Supersedes
[`drain-backpressure-scope.md`](drain-backpressure-scope.md) and the staging half of
[`ingest-scope.md`](ingest-scope.md); completes
[`../store/compaction-write-availability-scope.md`](../store/compaction-write-availability-scope.md)
by taking its lever 1 all the way.

Ingest used to store one sample by writing to the database **three times**. This change makes it
one. Staging — the table those extra writes existed to maintain — is removed entirely, along with
its bound, its overflow policy, its commit worker, its background reactor, and the four places a
caller drained it inline.

---

## 1. What staging was

A sample arrived at `ingest.write`. Instead of storing it in the `series` table where reads look
for it, the node first wrote it to a second table called `ingest_staging`. A background worker
called the **drain** later read those rows, wrote them into `series`, and deleted them from
`ingest_staging` in the same transaction.

Three writes for one sample:

1. the staging `UPSERT`,
2. the `series` `UPSERT` when the drain committed it,
3. the staging `DELETE`, which on this engine is itself a written record called a **tombstone** — a
   marker saying "this key is gone", appended like any other write.

---

## 2. Why it was believed to help

The original argument, stated in `ingest-scope.md`, was that the two writes are not the same price:

> a **staging append is cheap** where a **direct `series` write is expensive**: staging is one
> table with no secondary indexes, no rollup views and no tag edges.

An **index** here is a second lookup structure the database keeps so that a query can find rows by
something other than their primary key. The claim was that maintaining it costs real work on every
write, so a burst of samples should land somewhere without one and be moved into the indexed table
later, in batches, when the pressure had passed.

The scope was explicit that the relief was not about avoiding the disc:

> THE RELIEF IS NOT AVOID DISK.

---

## 3. Why the argument does not hold

**The landing zone was not cheap.** `ingest_staging` was a table in the same database as `series`.
A staged sample therefore paid the same write-ahead log append and the same in-memory write as a
committed one, and paid a tombstone when it left. Staging did not move work to a quieter moment; it
added work, and it added that work to the same database that was already under load.

**The indexed write it deferred was not expensive.** The database stores data in a structure called
an **LSM tree**. In an LSM tree every write — including every index entry — is appended to the same
write-ahead log and inserted into the same in-memory table, then written out in the background. An
index entry is simply another key and value. There is no index page to read, to lock, or to rewrite
in place, which is the cost the argument assumed. So the expensive write staging protected the node
from costs about one extra append.

**It was not backpressure.** Staging was also described as absorbing a burst the store could not
take. It cannot do that. If the store is too loaded to accept the write, staging responds by
writing to that same store two extra times. A buffer only relieves pressure when it sits somewhere
the pressure is not — in a producer's own memory, for instance, which belongs to the producer.

---

## 4. What was measured

200,000 samples, same machine, same store:

| Path | Time | Bytes written |
|---|---|---|
| Through staging | 115,398 ms | 11.87 MB |
| Committed directly | 3,752 ms | 4.02 MB |

Thirty times the time and three times the bytes. Of the staged cost, 93% was the per-sample staging
write itself.

---

## 5. What changed

`ingest.write` now calls `commit_direct` and nothing else. `commit_direct` splits the caller's
batch into chunks of `DIRECT_COMMIT_BATCH` (256) samples and commits each chunk in one transaction.

Removed:

| Removed | What it was |
|---|---|
| `ingest/src/write.rs` | the staged append |
| `ingest/src/overflow.rs` | the staging bound, drop-oldest eviction, overflow dead-lettering |
| `ingest/src/staging.rs` | replaced by `tables.rs`, which holds table names only |
| `host/src/ingest/drain.rs` | the commit worker and its bounded caller-path variant |
| `host/src/ingest/drain_lock.rs` | the per-workspace lock that serialised two drains |
| `host/src/ingest/drain_reactor.rs` | the background tick that drained the backlog |
| the four inline drains | in the MCP verb, the gateway route, the webhook accept, the federation mirror |

The table name `ingest_staging` stays on the store's reserved list. Nothing writes to it, but an
upgraded node can still be carrying the table on disc, and the name must not become claimable as
ordinary user data.

---

## 6. What the caller gets

Acceptance is now **stronger**, not weaker. Under staging, `ingest.write` returned once the sample
was durably staged, with its commit still pending; the sample was not readable until a drain ran.
Now the call returns once the transaction that stores the sample has committed, so the sample is
readable by the caller's very next read with nothing to wait for in between.

Exactly-once is unchanged and is what makes this safe. Every sample is written with `UPSERT` keyed
on `(series, producer, seq)`. A crash before the commit rolls the whole chunk back and the producer
never saw an acknowledgement, so a producer that re-pushes lands on the same rows.

Two properties genuinely go away, and both should:

- **The staging bound and its overflow policy.** There is no queue to bound. A producer that
  outruns the store now waits on the store, which is honest backpressure, and the write either
  succeeds or returns an error the producer can act on.
- **Recovery of accepted-but-uncommitted samples.** There were none to recover; a sample is now
  either committed or never acknowledged.

---

## 7. Two defects the removal exposed

**Dead letters had no reliable age.** A **dead letter** is a sample the node refused to store but
kept for an operator to inspect. Two things used to produce them: the staging overflow, which
stamped a `dead_at` field, and the series cardinality cap, which did not. With staging gone the cap
is the only producer, so every new dead letter arrived unstamped — and the retention pass falls
back to the sample's own timestamp, which comes from the producer and is not trusted. A producer
with a clock set to the future could make its dead letters permanent, in the one ingest table
nothing else bounds. The commit transaction now stamps `dead_at` on every diverted sample.

**Concurrent producers exhausted the retry budget.** Every commit reads a series' newest-sample
pointer and conditionally advances it. The database uses **optimistic concurrency**: two
transactions that touch the same row are both allowed to run, and the one that commits second is
rejected and told to retry. Producers writing to the same series all touch the same pointer row, so
several of them at once collide on every attempt. The old drain lock had been serialising these
commits and hiding the problem. Measured at six concurrent producers on one series, all 16 retries
were spent and a whole batch surfaced as an error.

Two fixes:

- **A per-workspace commit lock** (`ingest/src/commit_lock.rs`). Writers to one workspace take turns
  instead of colliding. It is held per transaction, not per push, so a producer sending 100,000
  samples never blocks a producer sending 10 for longer than one chunk — the coupling
  `drain-backpressure-scope.md` removed must not return through a new door.
- **A widened conflict matcher.** SurrealDB 3 reports some conflict-aborted transactions only as
  "The query was not executed due to a failed transaction", with the real cause swallowed. That
  message was not recognised as retryable, so a conflict the retry existed to absorb was surfaced
  as a hard error instead. It is now matched. The message is generic, so a transaction that failed
  for another reason is retried too; every caller here is idempotent, and the cost is the backoff
  budget before the same error surfaces anyway.

---

## 8. Upgrading a running node

A node upgrading from a build that staged may still hold rows in `ingest_staging`. There is no
drain any more, so those rows are not committed and are not readable.

In practice this window is very small: the drain reactor ran every two seconds, so a node that is
not actively receiving pushes has an empty staging table within seconds. **Leave a node idle for a
moment before upgrading it.** A node that is killed mid-backlog and then upgraded loses whatever
was still staged.

This was a deliberate decision, not an oversight. A one-shot boot drain was written and then
removed: keeping the staging read path alive to serve an upgrade leaves the whole idea in the tree
for the next reader to build on, and the idea is wrong.

---

## 9. Testing

Deleted, because they tested staging itself: `durable_redrain_test`, `ingest_drain_bound_test`,
`ingest_write_amplification_test`, `staging_cost_bench`, and the `crash_ingest` example.

Rewritten, because the property survives even though the mechanism did not:

- `ingest_conflict_storm_test` — was drain-versus-drain and drain-versus-GC; is now
  commit-versus-commit and commit-versus-GC. This is the test that found the retry exhaustion in §7.
- `series_normalize_test` — the regression in
  [`debugging/ingest/filtered-batch-stops-the-drain-loop.md`](../../debugging/ingest/filtered-batch-stops-the-drain-loop.md)
  was a commit loop that stopped when a pass committed zero rows. That loop is gone, but
  `commit_direct` has a chunk loop of its own and the same mistake there would silently drop every
  chunk after the first. The test now pushes three chunks that the operator's filter discards
  entirely and requires the reply to account for all of them.
- `series_dead_letter_gc_test` — produced dead letters through the staging overflow; now produces
  them through the series cardinality cap, which is the shipped path.

Everything else that used `write` + `commit_batch` to seed a test now calls `commit_direct`.

---

## 10. State

- `lb-ingest`: full suite green, including the rewritten conflict-storm test.
- Whole workspace compiles with `--all-targets`; `cargo fmt` clean.
- FILE-LAYOUT gate: no new violation, and one fewer than the parent branch.
- `lb-host`, `lb-role-gateway` and `lb-node` suites are **not yet run** — the local disc filled
  during the link step. They run in CI.
