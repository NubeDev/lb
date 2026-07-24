# Session — panel resolution negotiation (issue #101)

Status: **shipped (host slices 1+2 + rubix-ai UI half)**, 2026-07-24. v1.5 (structured `decimate` on
`federation.query`) **deferred** — see "v1.5 decision" below.

Scope (authority): [`../../scope/viz/panel-resolution-scope.md`](../../scope/viz/panel-resolution-scope.md)
· consumer half: rubix-ai `docs/scope/frontend/dashboard/panel-resolution-ui-scope.md`.
Reads with: [`../../scope/datasources/series-decimation-scope.md`](../../scope/datasources/series-decimation-scope.md)
(the shipped bucket contract this feeds), the query-acceleration pair (shared cache keys).

Issue: <https://github.com/NubeDev/lb/issues/101>.

---

## The ask (restated)

Every dashboard chart fetched **raw rows at full resolution**: the source-picker built
`{tool:"series.read", args:{series}}` bare (mode-less, no window, no budget) and `viz.query` **dropped**
the `maxDataPoints`/`minInterval` that ride the panel. Meanwhile the engine that solves it —
`series.read {mode:"buckets", from, to, width_ms}`, an O(buckets) spike-safe pushdown — was already
shipped. This session builds the **negotiation seam**: `viz.query` turns the panel's time range + point
budget into a bucket width and passes it down, to SurrealDB series natively and to federated tiles by
interval-macro pushdown. One contract, both engines, chosen per target by where the data lives.

## What shipped

### lb slice 1 — budget→width derivation + series bucket injection

- **`host/src/viz/resolution.rs`** (new, one responsibility): `derive_width(range_ms, budget,
  min_interval_ms)` — the step ladder `1s 5s 10s 30s 1m 5m 10m 15m 30m 1h 2h 3h 6h 12h 1d 7d 30d`,
  snap **UP** (a budget is a ceiling), `minInterval` floor, `MAX_BUCKETS` (2000) clamp; refuses a
  zero/inverted window cleanly. Table-driven unit tests pin the scope's headline (6h→30s, 1mo→1h,
  1y→12h, 2y→1d) and the budget-ceiling across a swept range. Also owns `resolution_for` (args→
  `{from,to,width_ms}`) and `maybe_inject_buckets` (the in-place upgrade).
- **Injection in `viz/query.rs::dispatch_target`** (one call each, after `apply_time_override`): a
  mode-less `series.read` target with a numeric window gains `{mode:"buckets", width_ms}` (the shipped
  decimation path). **Explicit author intent always wins** — an explicit `mode` (a `rows` table/export)
  or `width_ms` (aligned multi-series overlay) is left byte-for-byte. Default budget **1000**.
- **`viz/frame.rs`**: added `"buckets"` to `ROW_KEYS` (so a `{buckets:[…]}` result unwraps to N bucket
  rows, not one blob) and `"t"` (last) to `TIME_KEYS` (so a bucket frame gets a real Time axis). See
  the debugging entry.

### lb slice 2 — Grafana interval macros on `federation.query`

- **`host/src/viz/macros.rs`** (new): substitute `$__interval_ms` / `$__interval` / `$__timeFrom` /
  `$__timeTo` into a `federation.query` target's `sql` before dispatch, with the SAME derived
  resolution. **Zero SQL parsing** (pure textual replace); `$__interval_ms` replaced before `$__interval`
  (prefix-superset). **An un-macro'd SQL stays byte-identical** (a hand-SQL tile runs verbatim) — the
  load-bearing invariant, asserted against a real SQLite source.

### rubix-ai UI half (Slices A/B/C)

- **A — send the window**: `builder/panelResolution.ts` threads the shell-resolved window
  (`$__from`/`$__to`) onto a **chart** `series.read`/`federation.query` target (mode-less → the host
  buckets); a **non-chart** (table/stat) series target is pinned `mode:"rows"` so a table is never
  bucketed. Wired into `useVizQuery` → the window enters `vizFetchKey` (zoom refetches; the cache TTL
  directive stays OUT of the key). `queryOptions` already rode the panel.
- **B — `$__interval` computed built-in**: `lib/vars/intervalLadder.ts` mirrors lb's ladder + Grafana
  duration formatting; `builtins.ts` derives `$__interval`/`$__interval_ms` from `range/budget` when no
  interval *variable* is authored (an authored one still wins). Grafana-imported `$__interval`
  dashboards work unedited.
- **C — draw the envelope**: `views/timeseries/bucketFrame.ts` feature-detects the bucket frame shape
  (`{t,min,max,avg}`); `TimeseriesView` plots the `avg` line with a min–max band via a new `PlotSpec.band`
  carried through `buildPlot.wide()` and rendered as a translucent range `<Area>` in `PlotChart`. A
  plain sample frame (un-bumped lb pin) falls through to today's line — **graceful, feature-detected by
  frame shape, not version**.

## Decisions (recorded)

- **Default budget = 1000** (fixed), matching the shipped decimation examples and the ~1000-px chart —
  scope open question resolved.
- **Explicit `width_ms`/`mode` always wins** over injection (same rule as `timeFrom`) — resolved.
- **Macro spelling = Grafana-verbatim** (`$__interval` etc.) so the grafana-parity import path benefits
  for free — resolved.
- **Units = epoch ms.** `series.read` buckets take `from/to/width_ms` in ms; the derivation is in ms
  and the UI sends the window in ms. (The existing `viz_query_test` time-override test pins its series
  targets to `mode:"rows"` now — it tests override math, orthogonal to bucket injection.)
- **Envelope on-by-default** for line/area (scope open question) — a `fieldConfig` opt-out toggle is a
  small follow-up (not needed to ship the feature).
- **rubix-ai has no real-node vitest harness** (deliberately removed — `ui/vite.config.ts` excludes
  `*.gateway.test.tsx`, "rubix-ai is a thin embed with no such backend"). Per the repo's "absence is a
  decision" rule I did **not** re-materialize it. The real-node end-to-end contract is proven in lb's
  `viz_resolution_test.rs` / `viz_resolution_macros_test.rs`; the UI half is covered by pure unit tests
  (no mocks) for every derivation/detection helper + the hook test at the sanctioned `@/lib/ipc/invoke`
  transport boundary (the same pattern `useVizQuery.cache.test.tsx` uses).

## v1.5 decision — DEFERRED

The structured `decimate:{ts_col,value_col,from,to,width_ms}` arg on `federation.query`, compiled per
dialect (`time_bucket`/`date_bin`/`strftime`), was gated on the in-flight **fine-grained-data-path**
federation work (`crates/federation/src/{query,validate}.rs`, `source/postgres.rs`) being merged and
stable. Those commits are on master (`27ae3708 fix(federation): close direct-path data-loss +
LIMIT-pushdown regressions`, `2dc75c5b`), but (a) that "close regressions" commit signals the path is
still stabilizing, and (b) a concurrent session is active in this checkout. Per the scope's own
sequencing ("Land v1 host-side first; slice D's structured decimate waits for that branch to merge") and
the task's explicit permission, **v1.5 is deferred to its own focused session** against a
confirmed-stable federation crate. v1 already gives federated tiles the negotiation via the macro path;
"a negotiation seam nobody calls is not shipped" is satisfied end to end. Noted on issue #101.

## Test evidence (all green)

### lb — resolution + macros unit tests (`cargo test -p lb-host --lib viz::`)

```
running 33 tests
test viz::macros::tests::grafana_duration_formatting ... ok
test viz::macros::tests::interval_ms_not_eaten_by_interval ... ok
test viz::macros::tests::substitutes_all_four_macros ... ok
test viz::macros::tests::un_macrod_sql_is_byte_identical ... ok
test viz::macros::tests::substitute_in_place_and_skips_non_sql ... ok
test viz::resolution::tests::ladder_snaps_up_and_respects_the_budget ... ok
test viz::resolution::tests::budget_is_a_hard_ceiling_across_the_sweep ... ok
test viz::resolution::tests::min_interval_floors_the_width ... ok
test viz::resolution::tests::max_buckets_clamps_the_count ... ok
test viz::resolution::tests::empty_or_inverted_window_refuses ... ok
test viz::resolution::tests::injects_buckets_into_mode_less_window ... ok
test viz::resolution::tests::explicit_rows_mode_is_untouched ... ok
test viz::resolution::tests::explicit_width_is_untouched ... ok
test viz::resolution::tests::authored_budget_reaches_the_width ... ok
(+ frame/time_override tests)
test result: ok. 33 passed; 0 failed; 296 filtered out
```

### lb — resolution integration (real node, seeded series) `--test viz_resolution_test`

```
running 6 tests
test wide_window_returns_bounded_buckets_not_raw_rows ... ok
test spike_survives_in_bucket_max_at_the_dashboard_layer ... ok
test explicit_rows_mode_still_returns_raw_rows ... ok
test derivation_is_deterministic_and_zoom_refines ... ok
test bucket_injection_denied_without_series_cap ... ok
test bucket_injection_workspace_isolation ... ok
test result: ok. 6 passed; 0 failed; finished in 10.15s
```

### lb — macros over a REAL SQLite source `--test viz_resolution_macros_test`

```
{"evt":"federation.query","kind":"sqlite","outcome":"ok","rows":100,"source":"demo", …}
running 2 tests
test macro_target_denied_without_federation_cap_is_honest_empty ... ok
test macros_substitute_and_bucket_over_real_sqlite ... ok
test result: ok. 2 passed; 0 failed; finished in 10.51s
```

### lb — regression (no drift) `--test viz_query_test` + `--test series_plane_host_test`

```
viz_query_test:        17 passed; 0 failed; finished in 10.55s   (incl. panel_time_override_applies_to_target_dispatch)
series_plane_host_test: 5 passed; 0 failed                       (incl. bucketed_read_via_mcp_and_deny_without_cap)
```

### rubix-ai — UI unit tests (no mocks) + hook test (transport-stubbed)

```
✓ src/lib/vars/intervalLadder.test.ts (7)
✓ src/lib/vars/builtins.interval.test.ts (6)
✓ src/features/dashboard/builder/panelResolution.test.ts (12)
✓ src/features/dashboard/views/timeseries/bucketFrame.test.ts (5)
✓ src/lib/charts/buildPlot.band.test.ts (2)
  → 32 passed

✓ src/features/dashboard/builder/useVizQuery.resolution.test.tsx (3)   window in fetch key, zoom refetches, table pinned rows
✓ src/features/dashboard/builder/useVizQuery.cache.test.tsx (7)        (regression — TTL still out of the key)
  → 10 passed
```

Pre-existing, unrelated failures observed in the rubix-ai suite (`contract-mirrors.guard.test.ts`,
`widgetCatalog.consistency.test.ts`, `ExtHost.test.tsx` tsc lib-target) inspect files this session never
touched (`VizPicker.tsx`, devkit/echarts/thecrew theme mirrors) — not caused here.

## Files

lb: `host/src/viz/resolution.rs` (new), `host/src/viz/macros.rs` (new), `host/src/viz/mod.rs`,
`host/src/viz/query.rs`, `host/src/viz/frame.rs`; tests `viz_resolution_test.rs` (new),
`viz_resolution_macros_test.rs` (new), `viz_query_test.rs` (pinned mode:rows).

rubix-ai: `lib/vars/intervalLadder.ts` (new) + `builtins.ts`; `features/dashboard/builder/
panelResolution.ts` (new) + `useVizQuery.ts`; `features/dashboard/views/timeseries/bucketFrame.ts` (new)
+ `TimeseriesView.tsx`; `lib/charts/plotSpec.ts` + `buildPlot.ts` + `features/charts/PlotChart.tsx`;
tests colocated.

## Cross-links

- Debugging: [`../../debugging/viz/bucket-frame-collapses-to-one-blob-row.md`](../../debugging/viz/bucket-frame-collapses-to-one-blob-row.md)
- Public: `doc-site/content/public/datasources/datasources.mdx` (§ Panel resolution)
- Scope open questions updated in both scope docs.
