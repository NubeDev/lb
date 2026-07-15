# Ingest scope — a per-series sample cap (FIFO), a GC driver, and a safe default

Status: **release 1 code complete on a branch, testing INCOMPLETE — not shipped.** Tracked as
**[issue #65](https://github.com/NubeDev/lb/issues/65)**. Promotes to
`doc-site/content/public/ingest/ingest.md` once shipped — **not yet; nothing here is proven on a real
node.** What was built, what is verified, and what is still open:
[`sessions/ingest/series-sample-cap-session.md`](../../sessions/ingest/series-sample-cap-session.md).

A series can grow until the disc is full. We want a **per-series FIFO bound on stored samples** —
"keep at most N samples for this series; when N is exceeded, evict the oldest first" — so a
long-running producer costs a *bounded, predictable* amount of disc instead of an unbounded one.

Retention already ships a **time**-based horizon (`raw_for_ms` + rollup tiers, issue #58), but it
does not close the disc risk, for three reasons this scope fixes together:

1. **There is no count bound.** A policy can say "keep 24h" but not "keep 1M samples". At 10Hz that
   24h is 864,000 samples; at 1000Hz it is 86,400,000. Time does not bound bytes — **rate does**,
   and rate is the producer's choice, not the operator's.
2. **The GC has no driver.** `run_gc` is called only by tests and the on-demand
   `series.retention.gc` verb. **Nothing ticks it at boot.** So even a correctly-configured
   time-based policy evicts *nothing* on a real node unless someone manually calls the verb. This is
   the same missing-driver class as the ingest drain bug
   ([`drain-backpressure-scope.md`](drain-backpressure-scope.md)) — the mechanism shipped, its
   heartbeat didn't.
3. **The default is keep-forever.** No policy matches → nothing is ever evicted. A fresh node fills
   its disc with zero configuration and zero warning.

Any one of these alone means the disc fills. Fixing only the cap (1) would ship a limit that never
runs (2) on series nobody configured (3).

## The measured risk (why this is arithmetic, not a worry)

Measured on a real on-disk SurrealKV store, committed `series` rows with a scalar payload and a
realistic key spread:

| Committed samples | On disc | Per sample |
|---|---|---|
| 10,000 | 10.8 MB | ~1,077 bytes |
| 50,000 | 33.1 MB | ~663 bytes |

Call it **~700 bytes/sample** steady-state (the per-row constant falls as the store amortizes, and
excludes the `(series,seq)` / `(series,ts)` indexes' own growth). So:

| Workload | Disc/day | Fills a 64GB disc in |
|---|---|---|
| 50 series @ 1 sample/sec | ~3.0 GB | ~3 weeks |
| 200 series @ 1 sample/sec | ~12.1 GB | ~5 days |
| 50 series @ 10 samples/sec | ~30.2 GB | ~2 days |

The existing `DEFAULT_SERIES_CAP = 10_000` bounds **how many distinct series** exist, not how many
samples each holds — 10,000 series × unbounded samples is still unbounded. The staging bound
(`DEFAULT_STAGING_BOUND = 100_000`) bounds only the *uncommitted* landing zone. **Nothing bounds the
committed plane.** That is the hole.

## Goals

- **A per-series count bound with FIFO eviction.** `max_samples` on the retention policy: when a
  series exceeds it, the **oldest** samples are evicted first (by the plane's real time axis), down
  to the bound. Bounded disc per series, by a number an operator can multiply.
- **Actually run it.** A retention reactor at node boot, so the cap (and the existing time horizon)
  execute without anyone calling a verb. Without this the feature is decorative.
- **Safe by default.** With no policy configured, a default cap applies, so a fresh node cannot fill
  a disc by accident. An operator raises or disables it **deliberately**.
- **Compose with time-based retention, don't replace it.** `max_samples` and `raw_for_ms` are
  independent bounds on the same policy; a sample is evicted when it violates **either**. Time is
  "how old is too old"; count is "how much is too much". Real workloads want both.
- **Never silently lose must-deliver data without a trace.** Eviction is a policy decision, but it
  must be observable (counts per pass) — not an invisible drop.

## Non-goals

- **A byte/quota-based cap** (`max_bytes` per series/workspace). More directly what an operator
  wants ("use ≤10GB"), but it needs per-row size accounting the store doesn't cheaply expose, and
  row-size varies with payload. `max_samples` × measured bytes/sample is a good proxy today.
  Revisit if the proxy proves too loose.
- **Rate limiting** (bounding the *inflow*). Still open in `ingest-scope.md`. Complementary: this
  scope bounds what is *retained*, not what is *accepted*.
- **Changing the staging bound or the series cardinality cap.** Both already work; both bound a
  different axis.
- **A disc-full emergency mode** (panic-evict, read-only). If the bound works, we don't get there.
  Separate concern.

## Intent / approach

**One policy, three bounds, one driver.**

1. **`max_samples: u64` on `Policy`** (additive, serde-default `0` = unbounded — so every existing
   policy row keeps its exact meaning). GC gains a `cap_series` step: for each matching series,
   count rows; if `count > max_samples`, delete the oldest `count - max_samples` by the plane's
   ordering key. **Order by `(ts DESC, seq DESC)` to find what to keep** — the same axis
   `series.latest` uses. This is deliberate: per
   `debugging/ingest/latest-pinned-to-pre-restart-sample.md`, `seq` is monotonic **per
   `(series, producer)` only** and must never be compared across producers — a multi-producer series
   ordered by `seq` would evict the wrong rows. `ts` is the axis the streams share.
2. **Roll up before evicting, reusing the shipped path.** If the policy has `tiers`, the over-cap
   rows fold into them first (exactly as the time horizon does), so coarse history survives a cap
   eviction and `series.read {mode:"buckets"}` still renders. Cap-evict without tiers = real data
   loss, which is the operator's explicit choice.
3. **`spawn_retention_reactors(node, workspaces, period)`** in `ingest/retention_reactor.rs`,
   modelled on `drain_reactor.rs` / `relay_reactor.rs`, spawned from `node/src/reactors.rs`. Ticks
   `run_gc` on a slow cadence (minutes — retention is not latency-critical), `MissedTickBehavior::Skip`,
   errors logged-not-fatal, ws-scoped. **This is the piece that makes retention real.**
4. **A default policy when none matches.** `DEFAULT_MAX_SAMPLES = 100_000` applies to any series no
   policy covers — so a fresh node is bounded out of the box. `max_samples: 0` opts out explicitly.

   **Why 100k, not 1M** (an earlier draft proposed 1M; the arithmetic killed it). Three independent
   lines land on the same number:

   - **The workspace worst case is the number that matters.** The bound that counts is
     `max_samples × DEFAULT_SERIES_CAP (10,000 series)`, because that is what an unattended workspace
     can actually reach. At 1M that is **7 TB** — a "safe" default bigger than any disc we target is
     not safe, it is decorative. At 100k it is **0.7 TB**: still large, but a real ceiling that a
     real disc can be sized against.
   - **The realistic node.** 200 series at 100k = **14 GB** (fits a 64 GB Pi with room). The same
     node at 1M = **140 GB** — already over the disc, so the default would have failed exactly the
     workload it exists to protect.
   - **It matches the plane's own read ceiling.** `MAX_PAGE_LIMIT = 10_000` caps what one raw
     `series.read` page can ever return. 100k = ten full pages of raw history per series; beyond
     that, a caller is reading rollups regardless, so raw rows past ~100k earn little.

   **What 100k costs in history:** ~1.2 days at 1 sample/sec, ~11.6 days at 1/10s, ~69 days at
   1/min. A high-rate series that wants more sets a policy — which is the deliberate opt-in this
   design wants, and the cap makes that need *visible* (eviction shows up in the GC pass counts)
   rather than silently eating the disc. The failure mode is now "my old raw data aged out, I should
   set a policy", not "the node is dead".

   **Ship it in two steps (recommended).** 100k is tight enough that flipping it on silently would
   evict real data from a running deployment on the next boot. So: **release 1** ships the cap + the
   reactor + the default still `0` (unbounded), and **warns** when a series crosses 100k ("this
   series is unbounded and past the recommended cap; set a policy"). **Release 2** flips the default
   to 100k. Operators get a window to set real policies against a bound they can already see, and
   nobody's history vanishes on an upgrade they didn't read about. The risk to manage is that step 2
   gets forgotten — that is precisely how gap #3 (keep-forever) came to exist, so step 2 is part of
   this slice's definition of done, not a follow-up.

**Why FIFO and not a ring buffer.** A true ring would overwrite in place at write time (bounded
cost, no GC). Rejected: the sample id is `[series, producer, seq]` — a content-addressed dedup
identity, not a slot index — so "overwrite the oldest slot" has no meaning without a second index,
and it would break exactly-once re-drain. Evicting at GC keeps the write path cheap (the whole point
of the buffer) and reuses the shipped rollup-then-evict machinery.

**Why the cap is per-prefix, not per-series.** Policies are already prefix-keyed. `fleet.*` gets one
policy covering every series under it. Per-series overrides come free — a longer prefix wins.
**Open question below:** longest-prefix-wins needs to be specified; today `run_gc` iterates all
policies and a series matching two prefixes would be processed twice.

**Rejected alternatives:**

- *Cap at write time (reject/evict inline).* Rejected — it puts an O(count) query on every write,
  the exact coupling `drain-backpressure-scope.md` just removed. Retention belongs on the reactor.
- *Keep-forever default + a warning.* Considered seriously (zero behaviour change, never silently
  drops data). Rejected: a warning nobody reads doesn't stop a disc filling, and the failure mode of
  a full disc is a **dead node**, which is worse than bounded eviction on a series someone forgot to
  configure. Safe-by-default wins; the default is generous and disable-able.
- *A background compaction/vacuum.* Not the same problem — that reclaims space from deleted rows;
  this stops rows existing.

## How it fits the core

- **Tenancy / isolation:** policies, GC, and eviction are all workspace-scoped (`query_ws`), as
  today. A ws-B GC cannot touch ws-A rows; the reactor is ws-scoped like its siblings. Mandatory
  isolation test.
- **Capabilities:** no new verb, therefore **no new cap**. `max_samples` is a field on the policy
  `series.retention.set` already writes, gated by the existing admin-tier
  `mcp:series.retention.set:call`. The reactor mints no principal — it executes durable policy an
  authorized admin already wrote (exactly like the drain/relay reactors).
- **Placement:** `either` — the retention reactor is mounted by the ingest role, config not a code
  branch (rule 1), gated by `BootConfig::reactors` like its four siblings.
- **MCP surface:** unchanged verbs. `series.retention.set` gains an optional `max_samples` field;
  `series.retention.gc` gains `capped_raw` in its returned `GcPass`. Additive; existing callers
  unaffected.
- **Data (SurrealDB):** `series_retention` (one additive field), `series` (rows evicted),
  `series_rollup` (unchanged path). No new table.
- **Bus:** N/A — state-side.
- **Sync / authority:** eviction is node-local state. **Open question:** a series synced from a
  sub-hub — does the cap apply per-node (each bounds its own disc, correct for a Pi) or is eviction
  authoritative and replicated? Lean: per-node, since the disc being protected is per-node.
- **State vs motion:** unchanged; this only prunes state.
- **Secrets:** N/A.
- **One responsibility per file:** `ingest/retention_reactor.rs` (new, the driver);
  `lb-ingest/src/cap.rs` (new, the FIFO cap primitive, beside `gc.rs`/`rollup.rs`); `retention.rs`
  gains one field. Nothing near 400 lines.

## Example flow

A fleet of 200 sensors reporting every second into `fleet.*`, on a 64GB Pi:

1. An admin sets one policy: `series.retention.set {prefix: "fleet.", max_samples: 500000,
   raw_for_ms: 604800000, tiers: [{width_ms: 60000, keep_for_ms: 0}]}` — "at most 500k raw samples
   per series, and at most 7 days of raw, whichever bites first; keep 1-minute rollups forever."
2. Producers write. Each series grows past 500k samples in ~6 days.
3. The **retention reactor** ticks (no one calls anything). `run_gc` finds `fleet.*`:
   - the **time** horizon rolls up + evicts raw older than 7d;
   - the **count** cap rolls up + evicts the oldest rows of any series still over 500k.
4. Each series settles at ≤500k raw rows ≈ ~350MB. 200 series ≈ ~70GB… **which still doesn't fit** —
   so the operator lowers `max_samples` to 100k (≈70MB/series, ~14GB total) and the node is bounded,
   *by arithmetic they can do in advance*. That is the whole point: a number you can multiply.
5. A dashboard over the last hour reads raw; a dashboard over last quarter reads the 1-minute
   rollups that survived eviction. Nothing 404s.
6. A series nobody wrote a policy for (say `debug.probe`) is still bounded — by the 100k default
   (~70MB) — so a forgotten test producer cannot fill the disc.

## Testing plan

Real store/bus, no mocks (testing §0). Mandatory categories:

- **Capability deny** — `series.retention.set` with `max_samples` is refused without the existing
  admin cap (no new cap; assert the existing gate still covers the new field).
- **Workspace isolation** — a ws-B policy/GC never evicts ws-A rows; the reactor configured for ws-A
  leaves ws-B's series untouched (mirror `the_ingest_reactor_only_drains_its_configured_workspace`).

Slice-specific:

- **The headline: the cap evicts oldest-first and stops at the bound.** Write N samples, set
  `max_samples = M < N`, GC, assert exactly `M` remain **and that they are the NEWEST M** (assert
  identity, not just count — a cap that keeps the wrong M is worse than none).
- **Multi-producer safety (the trap).** A series with two producers whose `seq` spaces overlap and
  disagree with `ts`: assert eviction follows **`ts`**, not `seq`. Seed with `sample_at()` (which
  sets `ts` independently of `seq`) — per `debugging/ingest/latest-pinned-to-pre-restart-sample.md`,
  the `sample()` helper ties `ts: seq` and makes this bug class **unexpressible**. This test fails on
  a `seq`-ordered implementation.
- **Cap composes with the time horizon** — both set: whichever bites first wins; neither resurrects
  rows the other evicted.
- **Rollup-before-cap-evict** — with tiers, a bucketed read over cap-evicted history still returns
  data; without tiers, the rows are simply gone.
- **Idempotent** — a second GC pass at the same `now_ms` evicts 0 (the shipped GC's property; must
  survive).
- **The reactor actually runs** (the drain lesson: *a test asserting a plan never proves it
  executes*). Boot a node, exceed a cap, assert the series shrinks to the bound **with nobody calling
  `series.retention.gc`**. Not "assert spawn was called".
- **The default bound applies with no policy** — write past `DEFAULT_MAX_SAMPLES` on an unconfigured
  series, assert it is bounded. And that `max_samples: 0` explicitly means unbounded (opt out).
- **Disc actually stops growing** — the honest end-to-end: measure store size, write well past the
  cap, GC, assert size plateaus rather than climbs. (SurrealKV may not return space to the OS
  immediately — if it doesn't, **say so** and assert row count instead, documenting the caveat. A
  cap that bounds rows but not bytes is a partial win and must be reported as one.)

**Verification discipline** (per `verify-in-product-not-suite`): revert-check every regression test
and say so explicitly. Verify live: run a node with a small cap and a real producer, watch the series
plateau. `cargo test` has not caught the real bugs in this area.

## Risks & hard problems

- **Ordering by the wrong axis silently evicts the wrong data.** The single highest-risk item, and
  we have already been burned: `seq` is per-`(series, producer)` and must never order a series (that
  bug pinned `series.latest` to a stale sample for hours in production). If the cap orders by `seq`,
  a multi-producer series evicts *live* rows and keeps *dead* ones. Order by `ts`; test with
  independent `ts`/`seq`.
- **A default cap is a behaviour change, and 100k is tight enough to bite.** Existing deployments
  that kept everything start evicting at 100k/series — which at 1 sample/sec is only ~1.2 days of
  raw. That is the point (bounded beats dead), but it is the **sharpest edge in this slice** and must
  be **loud**: release note, `STATUS.md`, and the public doc, not a silent flip. Anyone who wants
  keep-forever sets `max_samples: 0`; anyone with a high-rate series sets a real policy — the cap
  makes that need visible instead of silent. **Consider shipping the default one release AFTER the
  cap+reactor** (cap available, default still unbounded, with a warning when a series crosses the
  threshold) so operators can set policies before anything evicts. Decide at implementation; if the
  two-step is taken, the default must not be forgotten — that is how gap #3 was born.
- **Counting rows per series per tick is not free.** `SELECT count()` per series per pass, with the
  store's global session mutex serializing every query
  ([`scope/store/session-concurrency-scope.md`](../store/session-concurrency-scope.md)) and up to
  10,000 series per workspace. A naive pass could be slow and hold the store. Mitigate: slow cadence
  (minutes), and consider counting only series that received writes since the last pass. **Measure
  before shipping the cadence** — `debugging/agent/dev-node-cpu-job-scan.md` is the precedent (a 2s
  reactor full-scanning a table burned 100% CPU).
- **Eviction cost on a big overshoot.** A series 10M rows over its cap deletes 10M rows in one pass.
  Batch the delete (the GC's keyset scan already has the shape) so one tick can't stall the store —
  the cap converges over several passes rather than one giant transaction.
- **`must-deliver` semantics.** QoS promises delivery, not immortality — but "must-deliver then
  cap-evicted" deserves an explicit statement in the public doc, or it reads as a broken promise.
- **The proxy is loose.** ~700 bytes/sample is payload-dependent; a series of large nested objects
  costs far more. `max_samples` bounds rows exactly and bytes only approximately. Say so.

## Open questions

- ~~**`DEFAULT_MAX_SAMPLES` value.**~~ **Answered: 100_000** (≈70MB/series; 0.7TB at the 10k-series
  worst case; 14GB on a realistic 200-series node; ten `MAX_PAGE_LIMIT` pages of raw). See Intent §4
  for the arithmetic that rejected the earlier 1M proposal — 1M × 10k series = 7TB, a default too big
  to fit any disc we target, and 140GB on a 200-series node, i.e. it would have failed the exact
  workload it exists to protect. Revisit only against a real deployment that measures it as wrong.
- **A per-workspace budget instead of a per-series cap?** Considered and deferred, not rejected:
  "this workspace uses ≤10GB" is closer to what an operator actually wants than a per-series count,
  but it needs live per-series size accounting and a fair-share eviction policy across series (whose
  data goes when the *workspace* is full?). The per-series cap is the tractable 80% today; a
  workspace budget is a natural follow-on once `series.list` can report sizes (see the last open
  question).
- ~~**Overlapping prefixes.**~~ **Answered in implementation: longest-prefix-wins.** Each series is
  governed by exactly one policy — its longest matching prefix (`governs()` in `gc.rs`), so a series
  under both `fleet.` and `fleet.eu.` is processed once, by the more specific policy. Note the
  precedence is **longest**, not **tightest**: a longer prefix with a *looser* bound is a deliberate
  override and wins. Regression-tested and revert-checked.
- **Per-node vs authoritative eviction** for a synced series (see Sync above). Lean: per-node.
  **Still open — untouched by release 1.**
- **Reactor cadence.** Shipped at `RETENTION_PERIOD = 300s`, `MissedTickBehavior::Skip`, not
  configurable. **This number is a guess, not a measurement — the open question stands.** This scope
  says *measure before shipping the cadence* and that has NOT been done: a `count()` per series behind
  the store's global session mutex, at up to 10k series/ws, is not free
  (`debugging/agent/dev-node-cpu-job-scan.md` is the precedent — a 2s reactor full-scanning a table
  burned 100% CPU). Measure against a realistic series count before trusting 300s. Whether the cap
  should tick more often than the rollup (cheaper query) is also still open.
- **Should `series.list`/`series.find` expose per-series row counts + est. bytes** so an operator can
  see what is growing before it bites? Adjacent, and arguably the thing an operator actually wants
  first. Possibly its own slice.

## Related

- [`series-retention-scope.md`](series-retention-scope.md) — the shipped time-based half this
  extends. **Read first**; this scope adds the count axis, the driver, and the default.
- [`ingest-scope.md`](ingest-scope.md) — the parent; "Retention / GC" and "Cardinality explosion"
  risks. This closes the count half of the former.
- [`drain-backpressure-scope.md`](drain-backpressure-scope.md) — the same missing-driver class
  (`drain_workspace` had no reactor; `run_gc` has none either). `ingest/drain_reactor.rs` is the
  implementation precedent for `retention_reactor.rs`.
- `debugging/ingest/latest-pinned-to-pre-restart-sample.md` — **why eviction must order by `ts`, not
  `seq`.** Load-bearing.
- `debugging/ingest/equal-seq-drain-order-nondeterministic.md` — tie-ordering on the drain key; the
  same "assert on a total order" discipline applies to which rows the cap evicts.
- [`../store/session-concurrency-scope.md`](../store/session-concurrency-scope.md) — why a
  per-series-per-tick query storm is a real cost.
- `crates/ingest/src/overflow.rs` — `drop_oldest`, the FIFO precedent at the staging bound (this is
  its committed-plane twin).
- README **§6.1** (the time-series model), **§3** rule 6 (workspace wall).

## Skill doc

**N/A** — no new drivable surface. `series.retention.*` verbs are unchanged; `max_samples` is an
additive field on an existing verb's payload. If a `skills/` doc ever covers retention
administration, it gains a field, not a new page.
