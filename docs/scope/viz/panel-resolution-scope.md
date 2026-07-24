# Viz scope — panel resolution negotiation (the dashboard finally asks for buckets, on both engines)

Status: **v1 shipped** (2026-07-24, slices 1+2 + rubix-ai UI half) — session
[`../../sessions/viz/panel-resolution-session.md`](../../sessions/viz/panel-resolution-session.md).
v1.5 (structured `decimate` on `federation.query`) **deferred** (see Open questions). Originally: scope
(the ask) — [issue #101](https://github.com/NubeDev/lb/issues/101). Consumer half:
rubix-ai `docs/scope/frontend/dashboard/panel-resolution-ui-scope.md`. Promotes to
`doc-site/content/public/datasources/` (beside the decimation contract it completes) once shipped.

Every dashboard chart today fetches **raw rows at full resolution**: the source-picker builds
`{tool:"series.read", args:{series}}` with no window, no mode, no budget — `mode:"rows"` by default —
and `viz.query` **drops** the two fields that were built to carry the intent (`QueryOptions.
maxDataPoints`/`minInterval` ride the panel model and the Rust struct but are never applied to a
target). Meanwhile the engine that solves this is **already shipped and fast**: `series.read
{mode:"buckets", from, to, width_ms|budget}` is an O(buckets) SurrealDB pushdown with spike-safe
`{t,min,max,avg,last,count}` buckets and the post-GC rollup-tier merge. This scope is the missing
**negotiation seam**: `viz.query` turns the panel's time range + point budget into a bucket width and
passes it down — to SurrealDB series natively, and to federated datasources as a pushed-down
`date_bin`/`time_bucket` GROUP BY. One contract, both engines, chosen per target by where the data
lives.

> Read with: `../datasources/series-decimation-scope.md` (the shipped bucket contract this feeds),
> `../datasources/series-read-perf-scope.md` (the O(buckets) pushdown), `../datasources/
> federation-paging-scope.md` (slice D — the federation half of the decimation contract; this scope
> is its first real caller), `../caching/dashboard-query-acceleration-scope.md` (cache + batch — the
> sibling levers; resolution composes with both), `grafana-parity-backend-scope.md` (maps Grafana's
> `interval` → `minInterval` on import — this scope is what makes that field DO something).

---

## Why a point budget, not a range→resolution table

The obvious design (and the first ask) is a fixed ladder — "1 month → 1 min, 1 year → 60 min,
2 years → 120 min". Rejected, for the reason the industry settled on the other model:

- **A fixed ladder ignores the pixel budget.** 1 month at 1-minute resolution is **43,200 points**
  into a ~1,000-px-wide chart — 40× the drawable data, paid on every load, every refresh, every
  viewer. The chart cannot show it; the wire and the browser still carry it.
- **The standard model (Grafana, Prometheus `step`, Timescale dashboards) derives the interval:**
  `width = ceil(range / maxDataPoints)`, snapped **up** to a human step ladder, floored by the
  panel's `minInterval` (never finer than the data's cadence). The resolution then scales with the
  range *automatically* — the user's instinct, without the table:
  | Range | budget 1,000 → snapped width | points |
  |---|---|---|
  | 6 h | 30 s | ~720 |
  | 1 month | 1 h | ~720 |
  | 1 year | 12 h | ~730 |
  | 2 years | 1 d | ~730 |
- Every window costs the same to render and roughly the same to ship. A per-panel `maxDataPoints`
  override covers the dense-sparkline and the big-wall-display cases; `minInterval` covers "my
  sensor reports every 15 min, never bucket finer".

The step ladder (final): `1s 5s 10s 30s 1m 5m 10m 15m 30m 1h 2h 3h 6h 12h 1d 7d 30d` — snap **up**
(a budget is a ceiling, never exceeded), clamp to the engine's `MAX_BUCKETS` (2,000).

## Goals

- **`viz.query` consumes `maxDataPoints`/`minInterval` + the panel time range** and derives one
  `(from, to, width_ms)` per time-series target — the fields exist end to end already
  (`dashboard/model.rs::QueryOptions`, the cell type in `packages/dashboard`); today they are
  authored, carried, and dropped.
- **Platform series targets are upgraded in place**: a `series.read` target with no explicit mode
  gets `{mode:"buckets", from, to, width_ms}` injected — the shipped decimation path, rollup-tier
  merge included, no engine work needed. An explicit `mode:"rows"` target is left alone (tables,
  exports, and the editor's raw view keep raw).
- **Federated targets get the same contract by pushdown** (the slice-D engine, first caller):
  - v1, zero SQL parsing: **interval macros** in the target SQL — `$__interval` / `$__interval_ms`
    / `$__timeFrom` / `$__timeTo` substituted by `viz.query` before dispatch, so an author writes
    `date_bin(INTERVAL '$__interval', ts, ...) GROUP BY 1` (the exact SQL shape the rules `rollup`
    verb already emits federation-side) and the query coarsens itself as the range grows. An
    un-macro'd SQL tile runs verbatim — untouched, today's behavior.
  - v1.5 (slice D proper): a structured `decimate: {ts_col, value_col, from, to, width_ms}` arg on
    `federation.query` that the child compiles to the source dialect (`time_bucket` on Timescale,
    `date_bin` on Postgres/DataFusion, `strftime` bucketing on SQLite) with the same
    min/max/avg/last bucket record as the platform engine — for the picker-built (non-hand-SQL)
    source bindings.
- **The dashboard sends the inputs**: the source-picker emits the window + inherits the page's
  `maxDataPoints` default (~1,000); the fetch hook threads the range on every refresh tick. (The
  rubix-ai half — same split as the query-acceleration pair.)
- **Deterministic width ⇒ cache-friendly**: the derived width is a pure function of
  `(range, budget, minInterval)`, so the quantised cache key (`cache/quantise.rs`) and the
  federation result cache collapse N viewers of the same panel onto one decimated compute —
  resolution composes with the acceleration scopes rather than fragmenting their keys.

## Non-goals

- **No new query engine, no new crate.** DataFusion stays exactly where it is (the federation
  child); it gains nothing here — bucketing must run **where the rows live** (SurrealDB `GROUP BY`
  / remote `date_bin`), which is the opposite direction from pulling rows into a local engine. The
  in-flight `fine-grained-data-path` work is *bypassing* DataFusion for simple queries; this scope
  rides that direct path, never re-inserts the ceremony.
- **No stored rollup tables as a cache** (rule 2, re-affirmed by the decimation scope). The only
  stored tier remains retention GC's post-eviction rollup.
- **No silent rewriting of hand-written SQL.** Decimation applies to series targets and macro'd /
  structured federated targets only. We never parse an author's SELECT to guess the time column.
- **Not the cache/batch levers** — `../caching/dashboard-query-acceleration-scope.md` owns those;
  this scope is the third lever (send less), designed to share its keys.
- **No LTTB in v1** (still the decimation scope's open question; the bucket record is the default).

## How it fits the core

- **Tenancy / capabilities: nothing new.** `viz.query` already re-enters every target through
  `call_tool_at_depth` under the caller's principal; an injected `mode:"buckets"` rides the same
  `mcp:series.read:call` grant, a macro-substituted SQL the same `federation.query` gate. Deny
  paths unchanged; the isolation tests re-assert through the new args.
- **Placement:** either — pure host/child code; no `if cloud`.
- **MCP surface:** additive only. No new verb: `viz.query` starts honoring fields it already
  accepts; `federation.query` gains the optional `decimate` field (v1.5). Explicitly stated so the
  half-built trap is avoided: v1 ships macros + series injection **and** the frontend sender —
  a negotiation seam nobody calls is not shipped.
- **Data / bus / sync / secrets:** untouched — read-time aggregation only.
- **One responsibility per file:** the derivation is one new file `host/src/viz/resolution.rs`
  (`derive_width(range, budget, min_interval) -> width_ms`, the step ladder, unit-tested
  table-driven); injection lives in the existing `dispatch_target`; macro substitution in
  `host/src/viz/macros.rs`.
- **Coordination:** the federation half touches `federation/src/query.rs`/`validate.rs`/
  `source/postgres.rs` — the same files as the in-flight `fine-grained-data-path` session. Land
  v1 (host-side only: injection + macros) first; slice D's structured `decimate` waits for that
  branch to merge.

## Example flow

A wall dashboard shows 12 panels of cooler temperatures over the last year; three viewers open it.

1. The page range is `[now-1y, now]`; the panel has no overrides → budget 1,000, `minInterval`
   unset. `viz.query` derives `width = 1y/1000 ≈ 8.8h → snap 12h`.
2. The `series.read` target is injected to `{mode:"buckets", from, to, width_ms: 43_200_000}` —
   SurrealDB returns ~730 spike-safe buckets in tens of ms; windows older than the raw horizon fill
   from the GC rollup tier transparently.
3. A Timescale tile on the same page wrote `... date_bin(INTERVAL '$__interval', ts, ...) GROUP BY 1
   ... WHERE ts BETWEEN $__timeFrom AND $__timeTo`; the macro substitutes `12 hours` and the
   warehouse aggregates — ~730 rows cross the wire, not 525,600.
4. All three viewers produce byte-identical derived args → the quantised subject-scoped cache and
   the federation result cache each compute once (single-flight), serve three times.
5. The user zooms to 6 h: the same derivation yields 30 s buckets — full detail, same ~720-point
   payload, no config anywhere.
6. **Deny/isolation unchanged:** a viewer without the series cap gets the same empty frame as
   today; ws-B resolves nothing from a ws-A series regardless of mode.

## Testing plan

Per `scope/testing/testing-scope.md` — real store, seeded real series, no mocks.

- **Derivation unit table** (`resolution.rs`): the ladder snaps up; budget is a hard ceiling at
  every range in the table above; `minInterval` floors; `MAX_BUCKETS` clamps; zero/negative ranges
  refuse cleanly.
- **Injection integration (real node):** a panel with a range + default budget over a 100k-sample
  seeded series returns ≤ budget buckets through `viz.query` (was: 10k raw rows capped) — and an
  explicit `mode:"rows"` target still returns rows.
- **Spike survives through the dashboard path** (re-assert the decimation scope's headline at this
  layer): the seeded spike shows in a bucket `max` in the assembled frame.
- **Macros:** substitution of all four; an un-macro'd SQL byte-identical to today; macro SQL against
  the real SQLite demo source returns bucketed rows; `$__interval` inside a string literal is the
  author's problem (documented), never a parse attempt.
- **Cache-key stability:** two calls with the same range/budget produce identical derived args
  (single-flight collapses them); a zoom changes the key.
- **Capability-deny + workspace-isolation** re-asserted through injected-args targets (mandatory).

## Open questions

- **Default budget:** ~~1,000 vs panel-pixel-width~~ **RESOLVED (2026-07-24): fixed 1,000**
  (`resolution.rs::DEFAULT_BUDGET`). A client pixel-width hint stays additive/future.
- **Injection when the target already carries explicit `width_ms`?** ~~Recommend explicit wins~~
  **RESOLVED: explicit `mode` OR `width_ms` always wins** (`maybe_inject_buckets` returns early) —
  same rule as `timeFrom`.
- **Macro spelling:** ~~Grafana verbatim vs house style~~ **RESOLVED: Grafana-verbatim**
  (`$__interval`/`$__interval_ms`/`$__timeFrom`/`$__timeTo`) — the grafana-parity import path benefits
  for free.
- **Bucket-shaped frames on the federation macro path:** **v1 ACCEPTED** — macros return whatever the
  author's SELECT yields (the SQLite demo test uses an explicit `max()`, so the spike survives); only
  the structured `decimate` (v1.5) *guarantees* the spike-safe record for picker-built bindings.
- **v1.5 — structured `decimate` on `federation.query` (per-dialect `time_bucket`/`date_bin`/
  `strftime`): DEFERRED (2026-07-24).** Gated on the in-flight `fine-grained-data-path` federation
  work being merged AND stable; its commits are on master (`27ae3708`) but still stabilizing, and a
  concurrent session is active in the checkout. Deferred to its own focused session per this scope's
  own sequencing ("land v1 host-side first"). v1's macro path already delivers federated decimation.

## Related

- `../datasources/series-decimation-scope.md` (shipped contract) · `../datasources/
  series-read-perf-scope.md` (shipped pushdown) · `../datasources/federation-paging-scope.md`
  (slice D, the federation engine this calls) · `../caching/dashboard-query-acceleration-scope.md`
  + rubix-ai `docs/scope/frontend/dashboard/dashboard-query-acceleration-scope.md` (the sibling
  levers + the frontend sender pattern) · `../ingest/series-retention-scope.md` (rollup tiers the
  bucketed read merges) · `grafana-parity-backend-scope.md` (`interval`→`minInterval` import
  mapping) · README §3 rules 2/3/5/6, §6.1.
