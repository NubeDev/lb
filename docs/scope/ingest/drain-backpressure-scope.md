# Ingest scope — bounded drain: a write must not pay for the backlog

Status: **SHIPPED 2026-07-15** (parts 1 + 2). See
[`sessions/ingest/drain-backpressure-session.md`](../../sessions/ingest/drain-backpressure-session.md)
and [`debugging/ingest/write-drains-whole-workspace-backlog.md`](../../debugging/ingest/write-drains-whole-workspace-backlog.md).

> **Part 3 (index the drain sort key) was NOT shipped — measurement overturned its premise.** The
> scope below required measuring before implementing; the numbers exonerated the `ORDER BY`. On a
> real on-disk store the drain is ~linear (4,096 rows = 735ms, 0.106→0.180 ms/row over a 16× growth),
> committed-table size is flat, and the only real effect found was distinct-series count (~2.3×, from
> `commit_batch`'s per-series round-trips — **not** the sort). An index would have taxed the
> deliberately index-free append to buy little. The sort is unchanged. Result of parts 1+2 at the
> reported 4,671-row backlog: **900.7ms → 66.0ms**, caller commits 256 rows not 4,672. The "why it's
> superlinear" reasoning retained below is **the original hypothesis, since disproven** — kept for
> the record, not as guidance.

`ingest.write` currently drains the workspace's **entire** staging backlog inside the caller's
call, so a producer pushing one sample to a backlogged workspace is billed for committing every
*other* producer's staged rows. Measured at pin `node-v0.4.5` against a real 1.4GB store: one
single-sample write against a 4,671-row backlog took **18,569ms**; the identical write immediately
after (backlog now 0) took **21ms**. The write itself is ~20ms — the other 18.5s is other people's
work. Worse, it is self-sustaining: a producer that times out abandons only the *wait*, the rows
stay staged, and the next push blocks again — so `accepted` never returns and the backlog only
grows. We want one property back: **a producer's write latency must not scale with another
producer's backlog.**

## Goals

- **Bounded write latency.** `ingest.write` costs O(own samples + one batch), never O(workspace
  backlog). No producer can make another producer's write slow.
- **Keep the write-then-read round-trip.** A sample written over a bridge must still be visible to
  the very next `series.latest`/`series.read` over that same bridge — the property `tool.rs:35-39`
  deliberately bought with the synchronous drain, and the one the proof-panel page proves. The fix
  must not trade a correctness-visible regression for latency.
- **Drain the backlog anyway.** Bounding the caller's share must not strand rows. The backlog
  drains — just not on a producer's call.
- ~~**Remove the superlinearity.** 4,671 rows costing ~18s is not 4,671 × 20µs; the per-batch
  `ORDER BY` over unindexed staging re-sorts the whole table on every one of the ~18 batches.~~
  **Dropped — disproven by measurement (see the banner).** The drain is ~linear on a real on-disk
  store; the sort is not the cost driver. The live 18.5s constant is that store's own per-op cost
  and is now off the request path. Chasing it needs the **live** store, not a synthetic one.
- **Exactly-once holds throughout.** The `(series, producer, seq)` UPSERT identity, one-batch =
  one-transaction atomicity, and must-deliver dead-lettering (never dropping) are unchanged.

## Non-goals

- **The store's global session mutex** (`store/src/open.rs:57`). Writes serialize perfectly
  linearly — 18 concurrent writers take 18× one writer (~96ms each, 1.7s wall). Real, measured, and
  a genuine scaling ceiling, but **not the cause of this bug** (1.7s is well under any sane budget,
  vs 18.5s here) and **deliberate**: it makes `use_ns(ws)` + query one critical section on a shared
  session with one mutable session. Removing it reintroduces the workspace-wall violation logged at
  `debugging/store/concurrent-use-ns-namespace-race.md` (it surfaced as a flaky login "not a member
  of any workspace"). Track it separately; any change there must keep the namespace wall airtight.
  **Do not touch it in this slice.** *(Now tracked at
  [`scope/store/session-concurrency-scope.md`](../store/session-concurrency-scope.md), with the
  ceiling independently reproduced: 18 concurrent writers, each its own workspace = 7.0ms = 18 ×
  0.4ms, zero parallelism. Confirms the report — and confirms it is not this bug: single-digit ms
  cannot make an 18-second call. The slice shipped with the mutex fully in place.)*
- **`Invalid revision NN for type Value` on a long-lived store.** Reported, not reproduced, no
  evidence. Not established; not scoped here. If a session hits it, open a debugging entry then.
- **Rate limiting** (still open in `ingest-scope.md`) and **producer-side staging bounds**. Both
  bound the *inflow*; this scope bounds the *drain*. Complementary, separately scoped.
- **A general job/queue rework.** The drain is already a durable staging set with an idempotent
  commit; it needs a driver, not a new mechanism.

## Intent / approach

**Finish the pattern ingest's own scope already named, and bound the caller's share of it.**

`ingest-scope.md` describes ingest as "the read-side analog of the outbox" and names a **commit
worker** "mounted by the ingest role"; `drain.rs`'s module doc says exactly that too. But the
worker was never given a driver — `drain.rs:29`'s comment is blunt: *"There is no background drain
worker."* So every caller became the worker, synchronously, unbounded. The outbox half of the same
pattern already has its driver (`outbox/relay_reactor.rs` → `node/src/reactors.rs`), and it is the
literal twin: durable staging, idempotent delivery, a detached per-node owner ticking a ws-scoped
scan, errors logged-not-fatal. This is a **missing-driver** bug, not a design rethink — the same
shape as the native-boot-respawn fix (`a498555`), where a plan's half was wired and the other half
wasn't.

**Three parts, in dependency order:**

1. **Bound the caller's drain.** `ingest.write` drains at most **one batch** (`COMMIT_BATCH`,
   today 256) — enough to commit the caller's own just-staged samples, which is what buys the
   round-trip. Introduce `drain_workspace_bounded(store, ws, max_batches)`; `drain_workspace`
   (drain-until-empty) stays for the reactor and tests. Write latency becomes O(batch).
2. **Give the worker its driver.** `spawn_ingest_reactors(node, workspaces, period)` in
   `ingest/drain_reactor.rs`, modelled line-for-line on `relay_reactor.rs`, spawned from
   `node/src/reactors.rs` beside the other four. It calls `drain_workspace` (unbounded) on a
   few-second cadence, so a backlog drains off every caller's path. Gated by `BootConfig::reactors`
   like its siblings — an embedder that opts out keeps today's synchronous behaviour.
3. ~~**Index the drain sort key.**~~ **RESOLVED: do nothing.** The plan was to index `(seq, ts)`
   because `commit.rs:153`'s `ORDER BY` over unindexed staging looked like the superlinear cost —
   with the explicit caveat that `staging.rs` keeps staging index-free *deliberately* (cheap append
   is the buffer's whole reason to exist), so it had to be measured, not assumed. It was measured,
   and the premise failed: drain cost is ~linear in backlog, flat in committed-table size, and only
   mildly sensitive (~2.3×) to distinct-series count — which points at `commit_batch`'s per-series
   `is_registered`/`register`/`apply_labels` round-trips, **not** the sort. So the index would have
   taxed the append path to buy little. The sort stays as-is; the ~2.3× per-series round-trip cost is
   a follow-up, not this slice. (Had the index been needed, the exit was to **drop the `ORDER BY`**
   instead — per `debugging/ingest/equal-seq-drain-order-nondeterministic.md` no caller-visible
   guarantee rests on drain order beyond "oldest-first, ties unspecified".)

Parts 1 and 2 are the fix — they change *who pays*, which is the bug. Part 3 would only have changed
*how much*, and turned out to be unnecessary: with the drain off the request path, the backlog's
absolute cost stopped being a caller's problem. **Shipping 1+2 was sufficient and complete.**

**Why one batch and not zero.** Draining *nothing* on the write path is cleaner but breaks the
round-trip: `POST /ingest`, the webhook accept path, and `ingest.write` all drain synchronously so
a just-written sample reads back immediately. One batch preserves that for the common case (a
caller writing ≤256 samples commits its own work) while capping the bill. A caller writing >256
samples in one call sees its tail commit on the reactor tick — name that in the public doc as the
explicit bound, don't hide it.

**Rejected alternatives:**

- *Drop the synchronous drain entirely, reactor-only.* Rejected — silently breaks write-then-read
  for four production call sites (`ingest/tool.rs:40`, `gateway/routes/ingest.rs:58`,
  `webhook/accept.rs:76`, `federation/mirror.rs:79`). That property is load-bearing and tested.
- *Make the drain a `lb-jobs` job.* Rejected — the drain is already durable and idempotent; a job
  wrapper adds a scheduling layer over a set that is its own queue. The outbox precedent (a
  reactor, not a job) is the house pattern for exactly this shape.
- *Keep the unbounded drain but time-box it.* Rejected — a wall-clock cut mid-backlog is
  nondeterministic and untestable; a batch count is a real, assertable bound.
- *Fix only the index (part 3).* Rejected — it makes the coupling ~10× cheaper without removing it.
  A big enough backlog re-creates the stall, and the self-sustaining trap survives.

## How it fits the core

- **Tenancy / isolation:** unchanged — the drain is already ws-scoped (`query_ws`) and the reactor
  ticks a configured workspace list, exactly like `spawn_relay_reactors`. A ws-B drain never
  commits into ws-A. Re-tested, not assumed.
- **Capabilities:** unchanged. `mcp:ingest.write:call` still gates the write; the gate still runs
  first (`authorize_ingest` in `write.rs`, before staging). The reactor is not a caller-facing
  surface and mints no principal — it drains what the gate already admitted, exactly as the relay
  reactor delivers what the gate already staged.
- **Placement:** `either`. The drain reactor is mounted by the **ingest role** — config, never an
  `if cloud`. A node that doesn't run it keeps the bounded synchronous drain and its staging simply
  isn't drained *there* (the "one authoritative ingest path per producer" rule already says this).
- **MCP surface:** **no new verbs, no wire change.** `ingest.write` keeps its `{samples: [...]}`
  shape and its `{accepted: n}` reply. This is a latency fix behind an unchanged contract — which
  is why it needs no SDK/WIT change either.
- **Data (SurrealDB):** `ingest_staging` (unchanged shape; possibly one new `(seq, ts)` index — see
  part 3), `series` (unchanged), `ingest_dead_letter` (unchanged). No new table.
- **Bus (Zenoh):** N/A — the drain is state-side. The live sample stream is untouched.
- **Sync / authority:** unchanged. Commit stays exactly-once on `(series, producer, seq)`; a
  restart still re-drains uncommitted staging; the reactor is just another caller of the same
  idempotent `commit_batch`. Two drainers racing (a write's bounded drain + a reactor tick) is
  **safe by the existing design** — the staged row is deleted in the same transaction as the series
  upsert, so the loser of a race commits nothing and re-reads next pass. Prove this, don't assume it
  (see testing plan).
- **Secrets:** N/A.
- **One responsibility per file:** `ingest/drain.rs` grows `drain_workspace_bounded` beside
  `drain_workspace` (one verb, two bounds — same responsibility); the driver is its own
  `ingest/drain_reactor.rs` (mirroring `outbox/relay_reactor.rs`); the index DDL joins the existing
  `ensure_series_schema` seam or a staging twin. No file approaches 400 lines.

## Example flow

Two producers, one workspace, a backlog — the measured scenario, after the fix:

1. Producer A (a bridge) has been pushing hard; 4,671 rows sit in `ingest_staging` for `ws=acme`.
2. Producer B calls `ingest.write` with **one** sample. `authorize_ingest` passes; the sample is
   stamped `{principal}/{declared}` and durably appended to staging. **~20ms.**
3. `tool.rs` calls `drain_workspace_bounded(store, ws, 1)`. One `commit_batch` of ≤256 rows runs in
   one transaction. B's own sample is in that batch (oldest-first by `(seq, ts)`; B's is committed
   within a bounded number of ticks regardless). **~one batch, not ~18 batches.**
4. `ingest.write` returns `{accepted: 1}`. **Total: tens of ms, not 18.5s.** B's next
   `series.latest` over the same bridge reads its value — the round-trip holds.
5. Meanwhile the ingest drain reactor ticks every few seconds, calling `drain_workspace` (unbounded)
   on `acme`, chewing the remaining backlog off everyone's call path. It logs
   `committed=N` per non-empty pass, exactly as the relay reactor logs `delivered=N`.
6. The backlog reaches 0. Nothing stalled, nothing was lost, no producer paid for another's rows.

## Testing plan

Real store, real bus, no mocks (CLAUDE §9). Mandatory categories from
`scope/testing/testing-scope.md`:

- **Capability deny** — a caller without `mcp:ingest.write:call` is still refused before anything is
  staged (unchanged path, re-asserted: the gate must not have moved relative to the bound). Per
  `debugging/auth-caps/schema-validation-preceded-cap-gate-leaks-400.md`, a deny test failing with a
  non-403 status means the test stopped testing the deny — don't "fix" the payload.
- **Workspace isolation** — a ws-B producer cannot write/read a ws-A series; **and the reactor is
  ws-scoped**: with backlogs in both ws-A and ws-B and the reactor configured for ws-A only, ws-B's
  staging is untouched.

Slice-specific — **the bug's own test comes first**:

- **The bounded-latency regression test (the headline).** Stage a large backlog (well over
  `COMMIT_BATCH` — e.g. 2,000 rows) directly into `ingest_staging`, then call `ingest.write` with
  **one** sample and assert it returns within a bounded budget. **Assert on batches committed, not
  only wall-clock** — a timing-only assertion is flaky on a loaded box (see
  `debugging/`'s `rules_test`-under-load note and the memory on shared-box artifacts). The honest
  bound is structural: *the call commits at most `COMMIT_BATCH` rows*. Pair it with a generous
  wall-clock ceiling as a smoke check, and if the wall-clock leg flakes, **say flaky** and keep the
  structural leg as the gate.
- **No loss under the bound** — after the write returns, the backlog + the new sample all commit
  (drive the reactor, or call `drain_workspace` explicitly) and every sample is present exactly
  once. Exactly-once must still hold; must-deliver still dead-letters rather than drops at the
  staging bound.
- **The round-trip survives** — write over the MCP bridge, then `series.latest` over the same
  bridge, no explicit drain: the value is there (the existing `host/tests/ingest_test.rs` property;
  it must stay green unmodified).
- **Concurrent drainers don't double-commit** — run a bounded write-path drain and an unbounded
  reactor drain against one workspace concurrently; assert each sample committed exactly once and
  none stranded. This is the atomic-dequeue claim under real contention.
- **The reactor actually drains** (the native-boot-respawn lesson: *a test asserting a plan never
  proves it's executed*). Boot a node with reactors on, stage a backlog, **assert staging reaches 0
  with no caller ever calling drain**. A test that only asserts `spawn_ingest_reactors` was called
  proves nothing.
- **Index/order decision, measured** — whichever way part 3 lands, record before/after numbers for
  both the drain *and* the staging append in the session doc.

**Verification discipline (non-negotiable, per `verify-in-product-not-suite`):** `cargo test` has
never caught the real bugs in this area. So: **revert-check every regression test** (confirm each
fails against the unfixed code, and say so explicitly in the session doc), **and verify live** —
against a **copy** of a backlogged store, time the *first* `ingest.write`. The repro is
**one-shot and self-concealing**: the first write drains the backlog, so a second probe returns
~20ms and looks perfectly healthy. Measure the first write or you have measured nothing. "Passed
once" is not "stable".

## Risks & hard problems

- **The one-shot repro.** The single biggest trap. Any measurement taken after a probe write is
  measuring a healthy store. Snapshot a backlogged store; measure once; re-copy to re-measure.
- **The staging-index trade is real.** `staging.rs` says "no secondary indexes" *by design* — it is
  the buffer's reason to exist. Adding one to fix the drain taxes the append the whole design
  optimizes. Don't wave it through: measure both sides, and take the drop-the-`ORDER BY` exit if
  the numbers say so.
- **The round-trip is easy to break silently.** Four production call sites depend on the
  synchronous drain, and the failure is invisible in unit tests that drain explicitly. The bridge
  round-trip test must run without an explicit drain, or it proves nothing.
- **Timing assertions flake.** A wall-clock bound on a shared box is an artifact generator. Prefer
  the structural bound; treat a timing flake as a harness signal, not a regression.
- **The reactor is a new always-on loop.** `dev-node-cpu-job-scan` is the cautionary tale — a 2s
  reactor full-scanning a big table burned 100% CPU. `commit_batch`'s drain is `LIMIT`-ed, so an
  empty-staging tick is cheap-ish, but confirm an idle node with a big `series` table doesn't spin.
  Pick the cadence deliberately (the relay's 2s is a reasonable start; a few seconds is fine —
  nothing here is latency-critical once the caller's own batch commits inline).
- **Backlog drain is still slow in absolute terms** until part 3 lands. That's acceptable — it's
  off the caller's path — but a 4,671-row backlog at ~18s per full pass means a reactor tick can
  overlap itself. Use `MissedTickBehavior::Skip` (as the relay does) and confirm passes don't pile.

## Open questions

**Resolved by the shipped slice (2026-07-15):**

- **Batch cap on the write path:** `ceil(own_samples / COMMIT_BATCH)`, floor 1 — the lean was taken.
  Shipped as `own_batches()` in `ingest/drain.rs`, exported once and used by all four caller paths
  (the MCP verb, the gateway route, the webhook accept, the federation mirror) so the rule has one
  home. A caller pays for its own data and nothing else.
- **Index the staging `(seq, ts)`, or drop the `ORDER BY`?** **Neither.** Measurement disproved the
  premise for both — see part 3 and the banner. The sort is untouched.
- **Reactor cadence and workspace list:** 2s and a single configured ws — matching all four siblings
  in `node/src/reactors.rs` exactly, no third convention invented.

**Still open:**

- **Should `ingest.write` report the residual backlog** (e.g. `{accepted, staged_remaining}`)? It
  would make the bound observable to a producer instead of silent. Additive and cheap — but it is a
  wire change, so decide deliberately. (Note the gateway's `POST /ingest` already returns
  `committed`, which now honestly reports *this request's* commits rather than the workspace total.)
- **The live 18.5s constant** — unexplained; a synthetic store doesn't reproduce it (this box shows
  900ms for the same 4,671-row backlog, ~20× smaller). It is now off the request path, so it is a
  curiosity rather than a defect. If it ever matters, profile the **live** store; the three obvious
  hypotheses (sort shape, committed-table size, series cardinality) are already exonerated.
- **`commit_batch`'s per-series round-trips** — the one real inefficiency the probes found (~2.3×
  from 1 → 1,024 distinct series): `is_registered`/`register`/`apply_labels` run sequentially per
  distinct series per batch, each taking the store's global session mutex. Batchable. Its own slice.

## Related

- `scope/ingest/ingest-scope.md` — the parent scope. Names the commit worker + the ingest role this
  slice finally drives; its "Backpressure / overflow at BOTH ends" test case is the sibling.
- `scope/inbox-outbox/outbox-scope.md` — the pattern this mirrors; `outbox/relay_reactor.rs` is the
  implementation precedent to copy.
- `scope/node-roles/` — the ingest role that mounts the reactor.
- `debugging/ingest/equal-seq-drain-order-nondeterministic.md` — why the drain's `ORDER BY` carries
  no caller-visible guarantee (load-bearing for part 3's exit).
- `debugging/ingest/latest-pinned-to-pre-restart-sample.md` — the last ingest bug that only a live
  store could show; same lesson, same discipline.
- `debugging/store/concurrent-use-ns-namespace-race.md` — why the session mutex (non-goal) exists.
- `scope/testing/testing-scope.md` §0 — no mocks; real store/bus.
- README **§3** (state vs motion, symmetric nodes), **§6.1** (the time-series model).

## Skill doc

**N/A.** This changes no drivable surface — no new MCP verb, no route, no wire change (unless the
`staged_remaining` open question lands, which would be an additive field on an existing verb, not a
new surface). `ingest.write` is already covered wherever the ingest verbs are documented; this
slice makes it faster, not different. If the public ingest doc gains a "what the drain guarantees"
note, that is a doc-site edit, not a skill.
