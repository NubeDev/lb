# Session — drain backpressure: `ingest.write` must not pay for the backlog

Status: **shipped** (backend). Scope: [`scope/ingest/drain-backpressure-scope.md`](../../scope/ingest/drain-backpressure-scope.md).
Debug entry: [`debugging/ingest/write-drains-whole-workspace-backlog.md`](../../debugging/ingest/write-drains-whole-workspace-backlog.md).
Spun out: [`scope/store/session-concurrency-scope.md`](../../scope/store/session-concurrency-scope.md) (tracking only).

## The ask

> `ingest.write` blocks the caller for as long as it takes to drain the ENTIRE workspace staging
> backlog, so any producer pushing to a backlogged workspace stalls for tens of seconds and its
> writes never confirm.

Reported live at pin `node-v0.4.5` (`a498555`) against a real 1.4GB SurrealKV store: one sample in →
**18,569ms**; the next identical write (backlog now 0) → **21ms**. Fix "whatever is best long term".

## What was actually wrong

Not a bad line — a **missing driver**. `ingest-scope.md` describes a "commit worker mounted by the
ingest role"; `ingest/mod.rs` called `drain_workspace` "the commit worker"; `drain.rs` admitted
*"There is no background drain worker."* Nothing ever ticked it. The synchronous drain is
load-bearing (it makes a write visible to the very next read over the same bridge — the proof-panel
round-trip), so with no reactor **every caller became the worker**, draining until staging was empty.

The outbox — the pattern ingest's own scope says it mirrors — has had its driver since day one
(`outbox/relay_reactor.rs` + `node/src/reactors.rs`). Ingest was the only durable-staging surface
without one. Same shape as `a498555` (native boot respawn): a plan whose half was wired and whose
other half wasn't.

`grep` for any ingest reactor returned **nothing**. Four production call sites paid the unbounded
drain, all on a caller's latency path — `ingest/tool.rs`, the gateway's `POST /ingest`,
`webhook/accept.rs`, and `federation/mirror.rs` (that one **inside a per-row loop**: re-draining the
whole backlog once per mirrored row, quadratic).

## What shipped

| Part | Change |
|---|---|
| **1. Bound the caller** | `drain_workspace_bounded(store, ws, max_batches)` + shared `own_batches(accepted)` = `ceil(accepted / COMMIT_BATCH)`, floor 1 — the ONE definition of "pay for your own work", used by all four call sites. `drain_workspace` (until-empty) stays, now reactor/tests only. Both share one loop (`drain_at_most`) so they can't drift on exactly-once. |
| **2. Drive the worker** | New `ingest/drain_reactor.rs` → `spawn_ingest_reactors`, modelled on `relay_reactor.rs` (`MissedTickBehavior::Skip`, ws-scoped, errors logged-not-fatal). Spawned from `node/src/reactors.rs` at 2s beside its four siblings; gated by `BootConfig::reactors` like them. |
| **3. Index the sort key** | **NOT done — the stated cause did not survive measurement.** See below. |

`own_batches` lives in `drain.rs` beside the bound it expresses and is exported once, so the rule has
a single home rather than four copies.

## Result — measured, real on-disk SurrealKV, the reported 4,671-row backlog

| | one-sample write | rows committed by the call |
|---|---|---|
| Unbounded (the bug) | 900.7ms | 4,672 (the whole backlog) |
| Bounded (the fix) | **66.0ms** | 256 (its own batch) |

13.6× here. The absolute numbers are ~20× below the live 18.5s because this repro store is fresh,
not 1.4GB — identical shape, smaller constant. The bound divides the constant out whatever its
origin: the caller pays for one batch, whatever a batch costs.

## The superlinearity claim was wrong (and measuring first is why we didn't ship a bad index)

The report blamed `commit.rs`'s `ORDER BY _seq, _ts LIMIT 256` over unindexed staging and proposed
indexing it. The scope required measuring first, because `staging.rs` documents "no secondary
indexes" as **deliberate** — the cheap-append relief the buffer exists to provide. Three probes:

- **vs backlog size** (on-disk): 256→4,096 rows = 0.106 → 0.180 ms/row. ~1.7× over a 16× growth —
  mildly super-linear, not a re-sort blowup. 4,096 rows drained in **735ms, not 18s**.
- **vs committed-table size** (backlog fixed at 1,024; `series` grown to 120k): 147ms → 216ms. Flat.
- **vs distinct-series count** (backlog fixed at 1,024): 0.100 → 0.250 ms/row across 1→1,024 series.
  A real ~2.3× from `commit_batch`'s per-series `is_registered`/`register`/`apply_labels`
  round-trips — the most interesting finding, and still far short of the reported magnitude.

So the `ORDER BY` is **not** the dominant cost. An index there would have taxed the deliberately
index-free append to buy little — and would have *looked* like the fix, because the real fix shipped
beside it. No index added; the sort left alone. The live 18.5s constant is that store's own per-op
cost (~20× this box); a synthetic store doesn't reproduce it. If someone wants the constant itself,
profile the **live** store — all three hypotheses here are exonerated.

## Tests + revert-check

`crates/host/tests/ingest_drain_bound_test.rs` — 5 tests, real store/bus, no mocks (testing §0):

1. `a_write_is_not_billed_for_another_producers_backlog` — the headline. **Structural, not
   wall-clock**: one sample vs a 2,000-row backlog must commit ≤ `COMMIT_BATCH`. A timing bound
   flakes on a loaded box and wouldn't pin *why* the call is fast.
2. `the_bounded_write_strands_nothing` — bounding who pays never changes what lands; exactly-once
   holds, re-drain is a no-op.
3. `the_ingest_reactor_drains_the_backlog_with_no_caller_involved` — asserts the **outcome**
   (staging → 0, nobody calls drain), never that a spawn fn was called.
4. `the_ingest_reactor_only_drains_its_configured_workspace` — the ws wall on the background path.
5. `a_writers_own_sample_reads_back_immediately_despite_a_backlog` — the round-trip a naive "delete
   the drain" fix silently breaks.

**Revert-check — both halves, verified explicitly:**

- Unbounded caller drain restored → test 1 FAILS: `the write committed 2001 rows — it must commit at
  most one batch (256)`.
- Reactor driver neutered (worker present, nothing ticks it = the exact pre-fix state) → tests 3+4
  FAIL: `the ingest reactor never drained the backlog — staging still holds 1000 rows`.

Green and unmodified: `ingest_test`, `ingest_isolation_test` (mandatory isolation),
`series_lifecycle_test`, `tags_test`, `proof_panel_test` — including
`ingest_write_then_latest_round_trips_through_the_bridge`, the property the fix had to preserve.
21/21 across the ingest-adjacent suites; `cargo fmt` clean.

Mandatory categories: **capability-deny** (`denies_write_without_capability` /
`denies_read_without_capability`, unmodified — the gate did not move relative to the bound);
**workspace-isolation** (`ingest_isolation_test` + the new reactor-scoped test).

## Spun out, not fixed: the store's global session mutex

`store/src/open.rs`'s `session: Arc<Mutex<()>>` serializes every query node-wide. Independently
reproduced: **18 concurrent writers, each its own workspace, take 7.0ms = 18 × 0.4ms** — perfect
linear serialization, zero parallelism, exactly as reported. But single-digit ms cannot make an
18-second call, and the mutex is deliberate: it makes `use_ns` + query one critical section, and
removing it reintroduces the cross-workspace leak in
`debugging/store/concurrent-use-ns-namespace-race.md`. It is the platform's next structural ceiling,
so it got its own tracking scope (`scope/store/session-concurrency-scope.md`) with the numbers, the
candidates (connection-per-ws leads; per-ws mutex explicitly rejected as wrong against one shared
session), and a recommendation to **spike before coding** — it does not bind at 0.4ms/op today.

Bug 3 (`Invalid revision NN for type Value`) was reported unverified and was not reproduced; not
chased, no entry opened.

## Follow-ups

- **`{accepted, staged_remaining}` on the write reply** — would make the bound observable to a
  producer instead of silent. Additive but a wire change; left open in the scope.
- **The live 18.5s constant** — unexplained and now off the request path. Profile the real store if
  it ever matters.
- **Per-series drain round-trips** — the one real inefficiency found (~2.3× at high series
  cardinality); `commit_batch` could batch its `is_registered`/`register` lookups.
- **Reactor cadence/ws list** — 2s and a single configured ws, matching its four siblings. If the
  siblings ever learn multi-ws enumeration, this follows.
