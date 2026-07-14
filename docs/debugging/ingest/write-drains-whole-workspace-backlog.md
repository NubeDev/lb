# `ingest.write` blocks for the whole workspace's staging backlog

- Area: ingest
- Status: **fixed**
- First seen: 2026-07-15 (live, pin `node-v0.4.5` / commit `a498555`, real 1.4GB SurrealKV store)
- Session: ../../sessions/ingest/drain-backpressure-session.md
- Scope: ../../scope/ingest/drain-backpressure-scope.md
- Regression test: `rust/crates/host/tests/ingest_drain_bound_test.rs`

## Symptom

A producer pushing to a **backlogged workspace** stalls for tens of seconds and its writes never
confirm. Measured live, same call, same node:

| Scenario | `ingest.write` (1 sample) |
|---|---|
| Fresh/empty store | 27ms |
| Real store, 4,671 staged rows | **18,569ms** |
| Immediately after (backlog now 0) | 21ms |

The write itself is ~20ms. The other ~18.5s is committing **other producers'** staged rows inside
the caller's call.

**Self-sustaining.** A client that gives up waiting abandons only the *wait* — the rows stay staged.
The producer keeps pushing, the backlog grows, every subsequent push blocks again. Steady state:
nothing ever confirms, staging only grows, and the producer's accept counter honestly reads 0
forever.

**One-shot and self-concealing** — the first write drains the backlog, so a second probe returns
~20ms and looks perfectly healthy. Any measurement taken after a probe write is measuring a healed
store. This is the main reason it survived to production.

## Cause

`ingest/tool.rs` called `drain_workspace` after staging the caller's samples, and
`drain_workspace` (`ingest/drain.rs`) loops `commit_batch` **until staging is EMPTY**. So the
caller paid to commit the entire workspace backlog — write latency was O(total workspace backlog),
not O(own samples).

The deeper cause is a **missing driver**, not a bad line. `scope/ingest/ingest-scope.md` describes a
"commit worker mounted by the ingest role"; `ingest/mod.rs` called `drain_workspace` "the commit
worker" — but nothing ever ticked it, and `drain.rs` said so outright: *"There is no background
drain worker."* With no reactor, the synchronous drain was load-bearing (it makes a just-written
sample visible to the very next `series.latest`/`read` over the same bridge — the proof-panel
round-trip), so **every caller became the worker**. The outbox — the pattern ingest's own scope says
it mirrors — has had its driver all along (`outbox/relay_reactor.rs` + `node/src/reactors.rs`).
Ingest was the one durable-staging surface without one.

**Four production call sites** were affected, all on a caller's latency path: `ingest/tool.rs`
(the MCP verb), `role/gateway/src/routes/ingest.rs` (`POST /ingest`), `webhook/accept.rs`, and
`federation/mirror.rs` — the last calling the unbounded drain **inside a per-row loop**, i.e.
re-draining the whole backlog once per mirrored row (quadratic).

## Fix

1. **Bound the caller's drain.** New `drain_workspace_bounded(store, ws, max_batches)` + a shared
   `own_batches(accepted)` = `ceil(accepted / COMMIT_BATCH)`, floor 1 — the one definition of "pay
   for your own work, never the backlog", used by all four call sites. `drain_workspace`
   (drain-until-empty) remains, now only for the reactor and tests. Both bounds share ONE loop
   (`drain_at_most`) so they cannot drift in exactly-once behaviour.
2. **Give the commit worker its driver.** New `ingest/drain_reactor.rs` →
   `spawn_ingest_reactors(node, workspaces, period)`, modelled on `relay_reactor.rs`
   (`MissedTickBehavior::Skip`, errors logged not fatal, ws-scoped), spawned from
   `node/src/reactors.rs` at 2s beside its four siblings. It drains the backlog unbounded — the
   reactor is exactly where an O(backlog) drain belongs, because it is nobody's request.
3. **The `ORDER BY` index: NOT done — the stated cause did not survive measurement.** See below.

## Result (measured, real on-disk SurrealKV, the reported 4,671-row backlog)

| | one-sample write | rows committed by the call |
|---|---|---|
| Unbounded drain (the bug) | 900.7ms | 4,672 (the whole backlog) |
| Bounded drain (the fix) | **66.0ms** | 256 (its own batch) |

13.6× on this box. The absolute numbers are ~20× smaller than the live 18.5s because the repro
store is fresh, not 1.4GB — but the *shape* is identical and the fix divides the constant out: the
caller now pays for one batch, whatever a batch costs.

## The superlinearity claim did NOT hold up (recorded so it is not re-chased)

The live report reasonably blamed `commit.rs`'s `SELECT … ORDER BY _seq, _ts LIMIT 256` over
**unindexed** staging, re-sorted per batch, and proposed indexing the sort key. Measured before
implementing (the scope required it, since `staging.rs` documents "no secondary indexes" as
deliberate — it is the cheap-append relief the buffer exists to provide):

- **Drain cost vs backlog size** (on-disk): 256→4,096 rows scaled 0.106 → 0.180 ms/row. Mildly
  super-linear, ~1.7× over a 16× size increase — **not** the 25×+ a `LIMIT`-less-style re-sort blowup
  implies. 4,096 rows drained in 735ms, not 18s.
- **Drain cost vs committed-table size** (backlog held at 1,024, `series` grown to 120k rows):
  147ms → 216ms. Essentially flat/noise.
- **Drain cost vs distinct-series count** (backlog held at 1,024): 0.100 → 0.250 ms/row from 1 to
  1,024 series — a real ~2.3× effect from `commit_batch`'s per-series `is_registered`/`register`/
  `apply_labels` round-trips, but still nowhere near the reported magnitude.

So the `ORDER BY` shape is **not** the dominant cost, and an index on staging's sort key would have
taxed the deliberately-cheap append to buy little. **No index was added**; the sort was left alone.
The 18.5s constant lives in the live 1.4GB store's own per-op cost (~20× this box's), which the
bound divides out regardless of its origin. If a future session wants the constant itself, profile
the **live** store — a synthetic store does not reproduce it, and none of the three hypotheses above
explains it.

## Regression tests + revert-check

`rust/crates/host/tests/ingest_drain_bound_test.rs` (5 tests, real store/bus, no mocks):

- `a_write_is_not_billed_for_another_producers_backlog` — **the headline.** Structural, not
  wall-clock: a one-sample write against a 2,000-row backlog must commit ≤ `COMMIT_BATCH` rows. A
  timing assertion would flake on a loaded box and would not pin *why* the call is fast.
- `the_bounded_write_strands_nothing` — bounding WHO pays never changes WHAT lands; every row
  commits exactly once and a re-drain is a no-op.
- `the_ingest_reactor_drains_the_backlog_with_no_caller_involved` — asserts the **outcome**
  (staging reaches 0, nobody calls drain), never that a spawn function was called.
- `the_ingest_reactor_only_drains_its_configured_workspace` — the ws wall holds for the background
  path (ws-B untouched).
- `a_writers_own_sample_reads_back_immediately_despite_a_backlog` — the round-trip a naive "just
  delete the drain" fix silently breaks.

**Revert-check (both halves, verified explicitly):**

- Restore the unbounded caller drain → `a_write_is_not_billed_for_another_producers_backlog` FAILS
  with `the write committed 2001 rows — it must commit at most one batch (256)`.
- Neuter the reactor's driver (worker exists, nothing ticks it — the exact pre-fix state) → both
  reactor tests FAIL with `the ingest reactor never drained the backlog — staging still holds 1000
  rows`.

Each half is independently proven. Also green, unmodified: `ingest_test`, `ingest_isolation_test`,
`series_lifecycle_test`, `tags_test`, `proof_panel_test` (incl.
`ingest_write_then_latest_round_trips_through_the_bridge`, the property the fix had to preserve).

## Not the cause: the store's global session mutex

`store/src/open.rs`'s `session: Arc<Mutex<()>>` serializes every query node-wide, and writes really
do scale linearly (independently reproduced: 18 concurrent writers, each its own workspace, take
7.0ms = 18 × 0.4ms — zero parallelism). But single-digit ms cannot produce an 18-second call, and
the mutex is **deliberate**: it makes `use_ns` + query one critical section, and removing it
reintroduces the cross-workspace leak in
[store/concurrent-use-ns-namespace-race.md](../store/concurrent-use-ns-namespace-race.md). Tracked
on its own at `scope/store/session-concurrency-scope.md`; untouched here.

## Lessons

- **A durable staging set with no driver makes every caller the worker.** The plan named a "commit
  worker" and the code called `drain_workspace` "the commit worker" — the words existed, the tick
  didn't. When a design says "background worker", grep for the thing that *spawns* it; a worker
  nothing calls is a function, not a worker.
- **A self-healing repro hides in production.** The first probe fixes the symptom, so every
  follow-up measurement reads healthy. Measure the first call against an untouched copy, or measure
  nothing.
- **Measure the stated cause before fixing it.** The `ORDER BY`/index diagnosis was plausible,
  well-argued, and wrong — three separate probes exonerated it. Fixing it would have taxed the
  append path (which `staging.rs` deliberately keeps index-free) to buy nothing, and would have
  looked like a fix because the *real* fix shipped alongside it.
- **Bounding who pays beats making the work cheap.** The drain's absolute cost is still unexplained
  on the live store — and it no longer matters on the request path, because no request waits for it.
