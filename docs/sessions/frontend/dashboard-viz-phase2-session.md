# Dashboard viz — Grafana-compatible visualization, Phase 2 (session)

- Date: 2026-06-29
- Scope: ../../scope/frontend/dashboard/viz/README.md (+ chart-types / field-config / panel-model / panel-editor sub-scopes)
- Stage: S9+ collaboration UI — additive over the shipped Phase-1 viz spine
- Status: done

## Goal

Ship Phase 2 of the viz slice — the rest of the everyday chart set (`stat`, `gauge`, `bargauge`,
`table`, `barchart`, `piechart`) end to end on the SAME v3 spine Phase 1 shipped. For each: one
renderer file, its typed per-viz `options` (Grafana names + defaults verbatim from
`/tmp/grafana/public/app/plugins/panel/<type>/panelcfg.cue`), the WidgetView switch + alias wiring, the
fieldConfig render path through the ONE user-prefs bridge (`fieldconfig/format.ts`), the editor
(viewOptions defaults + VizPicker + per-view PanelOptions editor), and result-shape↔type validation in
the picker. `reduceOptions` is the shared frame→value reducer for the single-stat-style panels.

Exit gate: a seeded v2 `stat`/`gauge`/`table` cell renders through the new renderer and re-saves
identically; each Phase-2 view's typed `options` round-trips through `dashboard.save`/`get`; the picker
offers only views a target's data shape can honestly fill; every value formats through the one bridge;
mandatory deny + workspace-isolation green; add≡edit parity holds (the cellEditorState round-trip
extended to all six views).

## Invariants held (unchanged from Phase 1)

- **A — one data hook.** Every Phase-2 renderer reads rows ONLY through `usePanelData`. No direct
  `bridge.call` in any renderer/editor. Phase 3 swaps that hook's body to `viz.query`.
- **B — no client-side transform lib.** Phase 2 adds NO transforms. `reduceOptions` for stat/gauge/
  bargauge/piechart is a per-viz frame→value reducer for SINGLE-STAT panels (collapse a frame to the
  value those panels draw) — NOT the transform pipeline, which is born backend in `lb-viz` (Phase 3).

## Plan (files, order, reduceOptions, shape validation)

- **Shared:** `views/reduce.ts` (the `reduceOptions` frame→value bridge — `reduceFrame`/`reduceFrameValues`/
  `frameCategories` + the calc set, reused by the timeseries legend), `views/shape.ts` (the conservative
  result-shape detector + `viewsForShape`/`viewFitsShape`), `views/field.ts` (the value field's resolved
  options + threshold/fixed/palette color — used by all single-stat panels).
- **Per view** (one renderer + one `options.ts` each, Grafana names+defaults verbatim):
  `stat/`, `gauge/`, `bargauge/` (+ `Bars.tsx`), `table/`, `barchart/`, `piechart/`.
- **Recharts helpers extended** (`widgets/recharts.tsx`): `GaugeArcChart` gains a `color`; new
  `PieChartSvg`, `CategoryBarChart`. (bargauge bars are view-specific markup → `bargauge/Bars.tsx`.)
- **Wiring:** `WidgetView.tsx` switch routes the six; `editor/viewOptions.ts` +6 defaults; `VizPicker.tsx`
  shape-filtered + buildable; `tabs/PanelOptionsTab.tsx` → dispatcher to `tabs/options/<view>.tsx` (+ shared
  `controls.tsx`/`ReduceOptionsEditor.tsx`; timeseries extracted to `TimeseriesOptionsEditor.tsx`);
  `editor/usePanelShape.ts` feeds the picker via the one data hook; `cellEditorState.ts` `OWNED_OPTION_KEYS`
  extended so Phase-2 option keys round-trip in typed groups.
- **Build order:** stat → gauge → bargauge → piechart → barchart → table (stat first exercises
  reduceOptions + the full single-stat surface + fieldConfig formatting; each wired + green before the next).
- **reduceOptions** lives in `views/reduce.ts` (one file, reused everywhere). **Shape validation:**
  `usePanelShape(draft)` reads rows through `usePanelData` (invariant A) → `detectShape` → `VizPicker`
  disables views the shape can't honestly fill; `reduceOptions` is the visible scalar/series→value bridge.

## What changed

### New shared view logic (`views/`)
- `reduce.ts` — `ReduceDataOptions` (Grafana, default `{calcs:[]}`) + `reduceFrame`/`reduceFrameValues`/
  `frameCategories`/`reduceCalc`. `timeseries/Legend.tsx` now imports `reduceCalc` from here (dedup).
- `shape.ts` — `detectShape` (scalar/series/table/unknown, conservative) + `viewsForShape`/`viewFitsShape`.
- `field.ts` — `valueFieldOptions`/`valueColor`/`categoryColor` (+ the classic palette) for the single-stat
  + categorical panels — the one place a value→color/format is resolved.

### Six renderers + their options (one file each)
- `stat/{options,StatPanel}`, `gauge/{options,GaugePanel}`, `bargauge/{options,BarGaugePanel,Bars}`,
  `table/{options,TablePanel}`, `barchart/{options,BarChartPanel}`, `piechart/{options,PieChartPanel}`.
- `widgets/recharts.tsx` — `GaugeArcChart` gains `color`; new `PieChartSvg` + `CategoryBarChart` (+ shared
  `CategoryDatum`/`tooltipStyle`).
- `views/WidgetView.tsx` — switch routes `stat/gauge/bargauge/table/barchart/piechart` to the new panels.
- **Deleted** the dead v2 `views/StatView.tsx`/`GaugeView.tsx`/`TableView.tsx` (replaced by the panels; no
  remaining importers — FILE-LAYOUT: no dead files).

### Editor (extended, not forked)
- `editor/viewOptions.ts` +6 defaults; `editor/VizPicker.tsx` rewritten (buildable + `shape`-filtered);
  `editor/usePanelShape.ts` (new — shape probe over the one data hook); `editor/PanelEditor.tsx` passes the
  detected shape to the picker; `editor/cellEditorState.ts` `OWNED_OPTION_KEYS` extended with the Phase-2
  typed option keys.
- `editor/tabs/PanelOptionsTab.tsx` → thin dispatcher; new `editor/tabs/options/` =
  `controls.tsx` (shared SelectField/ToggleField/NumberField), `ReduceOptionsEditor.tsx`,
  `TimeseriesOptionsEditor.tsx` (extracted from the old PanelOptionsTab), and one editor per Phase-2 view.

## Decisions & alternatives

- **`reduceOptions` is one shared reducer, not per-view copies.** stat/gauge/bargauge/piechart all collapse
  a frame the same way; `views/reduce.ts` owns it and the timeseries legend reuses `reduceCalc`. *Rejected:*
  duplicating the calc switch per renderer — it drifts (the legend already had its own copy; now deduped).
- **The shape detector is conservative + the picker disables (not hides) bad fits.** A multi-column frame is
  `table`-only (a gauge would have to guess a number); a single numeric row is `scalar`; `unknown` (no data)
  stays permissive. Disabling-with-a-reason is more honest than hiding (the user sees the view exists + why
  it doesn't fit). *Rejected:* an implicit "guess the number" that fabricates a value — banned by the
  no-mock rule; `reduceOptions` is the explicit, visible bridge instead.
- **bargauge `Bars.tsx` is a view file, not a recharts helper.** It's CSS fill markup (gradient/lcd/basic),
  not an SVG chart — recharts adds nothing. The genuine SVG charts (pie, category bars) ARE recharts helpers.
- **The v2 stat/gauge/table views are deleted, not kept as a fallback.** Their canonical id IS themselves
  (no alias indirection like chart→timeseries), so the new panel renders a v2 cell directly; keeping the old
  files would be dead code. The alias-fidelity test proves a seeded v2 cell renders + re-saves identically.
- **Per-view options editors are one-file-each under `tabs/options/`** (incl. extracting the Phase-1
  timeseries editor there) so `PanelOptionsTab` is a thin dispatcher (FILE-LAYOUT, ≤400 lines, one
  responsibility). Shared controls (`controls.tsx`) carry the justified native-control disables (the
  Phase-1 precedent — no shadcn Select/Checkbox primitive yet).
- **`usePanelShape` reads through `usePanelData`** (not a new fetch) so invariant A holds and the Phase-3
  `viz.query` swap stays a one-file change.

## Tests

Real gateway/store, seeded real rows (no mocks — CLAUDE §9). Mandatory categories: capability-deny (across
stat/gauge/table) + workspace-isolation; plus alias fidelity, options round-trip, result-shape↔type
validation over real samples, and fieldConfig-through-the-one-bridge (no stored formatted string).

- **UI unit** `pnpm test` — **147 passed (22 files)**. New: `editor/cellEditorState.test.ts` +1 (full
  stat/gauge/bargauge/table/barchart/piechart cells round-trip with typed options) and
  `views/viz.phase2.test.ts` (8 — reduceFrame/values/categories + detectShape/viewsForShape/viewFitsShape).
- **UI gateway** `pnpm test:gateway viz.phase2` — **6 passed**: alias fidelity (v2 stat/gauge/table render
  through the new renderer + re-save identically), options round-trip (all six views through save/get),
  result-shape↔type over real seeded samples (1-sample scalar→stat/gauge not timeseries; multi-sample
  series→timeseries+single-stat; reduce collapses to one value), fieldConfig one-bridge (42→"42.0" computed
  at render, no stored string), capability-deny (denied series.read → honest denied across stat/gauge/table),
  workspace isolation. Full `pnpm test:gateway` — 157/158 (the 1 fail is the documented pre-existing
  SystemView flake; passes isolated 9/9).
- **Rust** `cargo test -p lb-host` green (dashboard_test 10/10 unchanged — host stores Phase-2 options
  opaquely, no backend change); gateway `dashboard_routes_test` 6/6; `cargo build --workspace` + `cargo fmt`
  clean.
- **Quality gates:** `pnpm exec tsc --noEmit` clean; `pnpm lint` — new files clean (the 3 error files
  `VariableEditor`/`StudioView`/`WorkspaceSwitcher` are pre-existing, no diff from HEAD).

### Green output (key runs)

```
ui$ pnpm test                       → Test Files 22 passed (22) | Tests 147 passed (147)
ui$ pnpm test:gateway viz.phase2    → Test Files 1 passed (1)  | Tests 6 passed (6)
ui$ pnpm test:gateway               → 157 passed, 1 pre-existing SystemView flake (9/9 isolated)
ui$ pnpm test:gateway SystemView    → Tests 9 passed (9)   (confirms the flake is pre-existing)
ui$ pnpm exec tsc --noEmit          → clean
rust$ cargo test -p lb-host --test dashboard_test          → ok. 10 passed; 0 failed
rust$ cargo test -p lb-role-gateway --test dashboard_routes_test → ok. 6 passed; 0 failed
rust$ cargo build --workspace && cargo fmt                 → clean
```

## Debugging

None opened — no defect required a `debugging/` entry. One in-build correction (not a shipped bug): the
first gateway-test draft used `store.query` literals (`SELECT 42 AS value`), which the host's SELECT
parse-allowlist rejects (SurrealDB requires a `FROM`). Switched the seeds to the real `seedSeries` →
`series.read` path (the Phase-1 precedent) — caught + fixed within the session, before any commit.

## Public / scope updates

- Promoted to `docs/public/frontend/dashboard.md` — new "Grafana-compatible visualization — Phase 2" section.
- Marked Phase 2 shipped in the viz README phasing + the chart-types-scope status line + phasing.
- `STATUS.md` — new Phase-2 shipped slice row.

## Follow-ups

- Phase 3: `viz.query` + `lb-viz` (swap `usePanelData`'s body) + the real transform pipeline + multi-
  datasource targets + the visx/markdown panel family (histogram/state-timeline/status-history/heatmap/text).
- Phase 4: Grafana JSON import/export + `schemaVersion` migration.
- `lb-prefs` has now shipped — a follow-up swaps `fieldconfig/format.ts`'s fallback for the real `format.*`
  call (no schema change, no re-save; the `viaPrefs` flag is the swap point). All Phase-2 panels inherit it
  for free (they format through the one bridge).
- Generate shadcn `Select`/`Checkbox` primitives so `tabs/options/controls.tsx` drops the native-control
  disables.
- STATUS.md updated: yes.
</content>
</invoke>
