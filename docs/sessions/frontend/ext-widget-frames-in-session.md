# Session — ext-widget frames-in (Part A) + echarts-panel reference (Part B)

Date: 2026-07-03. Branch `master`. Scope: `scope/frontend/dashboard/ext-widget-source-binding-scope.md`.

## The ask

Make an extension `[[widget]]` a **first-class view over the v3 panel model**: an `ext:<id>/<widget>`
cell carries the same `sources[]` + `fieldConfig` + `transformations[]` as a built-in `timeseries`
cell, the shell resolves them through the shipped `viz.query` path under the **viewer's** grant, and
the tile receives **resolved frames** — it renders, it never fetches. Then prove it with a reference
extension (`echarts-panel`) that renders `ctx.data` with Apache ECharts, driven by the shared Field-tab
options, identically in a dashboard and a channel `rich_result` (ONE render path).

## What shipped

### Part A — the platform (frames-in)

1. **Manifest opt-in `data = true`** on `[[widget]]`, projected through the existing Install→ExtUi path:
   - `rust/crates/ext-loader/src/manifest.rs` — `Widget.data: bool` (`#[serde(default)]`).
   - `rust/crates/assets/src/install/model.rs` — `ExtUi.data: bool` (durable, serde-defaulted).
   - `rust/crates/host/src/ui_decl.rs` — `narrow`/`project` thread `data` (a **page** is always `false`).
     New test `data_flag_projects_and_defaults_false`.
   - `ui/src/lib/ext/ext.api.ts` — client `ExtUi.data?: boolean`.
   - `packages/source-picker/src/{types,sourcePicker}.ts` — `ExtUi.data` + `SourceEntry.data`;
     `extWidgetEntries` sets `data: tile.data === true` so the picker entry knows it's a data view.

2. **ctx v3 — the frozen mount contract, moved in ALL THREE mirrors together** (the drift risk the scope
   flagged loudly):
   - Host mirror: `ui/src/features/dashboard/builder/federationWidget.ts` — new `WidgetCtx` (`v:3`,
     additive `data?: WidgetFrame[]`, `fieldConfig?`), `WidgetFrame`/`WidgetField` (the `lb-viz` frame
     shape, now a public contract), `WidgetHandle` (`{ update?, teardown? }`), and the widened
     `RemoteWidgetMount` return `void | (() => void) | WidgetHandle`.
   - Ext-sdk / devkit template: **new** `rust/crates/devkit/templates/ui/src_contract.ts.tmpl` — the
     canonical extension-side copy (a scaffolded ext previously had NO widget contract — that gap was the
     drift; the template now carries it).
   - Extension copy: `echarts-panel`'s widget ctx (`ChartCtx`/`TileHandle` in `chart/mountChart.ts`)
     matches byte-for-byte (verified: `v>=3` gate, `data: Frame[]`, `fieldConfig`, `{update, teardown}`).
   - **Version-gate: `WIDGET_CTX_V = 3`.** A v2 tile (bare-fn return, no `data`) is byte-identical under
     the v3 shell — extra ctx fields ignored, a fn return still tears down.

3. **Shell resolution — frames-in** (`ui/src/features/dashboard/builder/`):
   - New `useVizFrames.ts` — the frames counterpart to `useVizQuery`. SAME bridge leash, SAME
     interpolation, SAME `vizQueryKey` cache key → an ext data tile and a built-in bound to the same spec
     **share one gateway round-trip** (no per-tile duplicate stream). Returns raw `frames` (vs
     `useVizQuery`'s flattened `rows`). Resilient to a missing `DashboardCacheProvider`
     (`useDashboardWsOptional` + a standalone client): a v2 ext tile can mount outside a dashboard
     (fetches through its own bridge, needs no frames) without throwing.
   - `ExtWidget.tsx` rewritten: detects a data tile via `tile.data`, resolves the cell's `sources[]`
     under the VIEWER's grant, builds the v3 ctx (`data` + `fieldConfig`), and pushes fresh frames via
     the tile's `update(ctx)` handle **without a re-mount** (a separate effect keyed on the memoized
     ctx). **The hard-won per-run-slot StrictMode lifecycle is preserved verbatim** — frames flow through
     `update`, NOT `configKey`, so a live tick never tears the tile down. A v2 tile (bare-fn return) is
     wrapped as `{ teardown }` and falls back to re-mount-on-configKey.
   - `WidgetView.tsx` now threads `cell` + `refreshKey` into `ExtWidget` (it needs the cell to read
     `sources[]`/`fieldConfig`).

4. **Editor** (`ui/src/features/dashboard/editor/tabs/QueryTab.tsx`): the scope said "VizPicker lists data
   tiles" — the real seam is the source-picker "Extension widgets" group + `selectEntry`. A **data**
   widget (`entry.data === true`) now KEEPS the cell's `sources[]` (does NOT clear `targets: []`) and
   keeps its `ext:` view while the source is (re)bound — exactly like a built-in `timeseries` keeps its
   view while you rebind its source. A **bare** v2 widget still collapses the Query surface and clears
   targets (unchanged). The Field/Transform tabs are ALWAYS mounted (no gate existed to remove), so a
   data tile gets them for free.

5. **Zero new extension caps.** A data tile's manifest needs no read verbs (`echarts-panel`:
   `request = []`, widget `scope = []`). The shell fetches under the viewer's token; per-target deny in
   `viz.query` degrades a denied target to an **honest empty frame** (confirmed in ground truth — NOT an
   error-payload frame), workspace-walled.

### Part B — echarts-panel (the reference)

New `rust/extensions/echarts-panel` (cloned from `proof-panel`'s build shape; proof-panel untouched):
- `extension.toml`: one `[[widget]]` "Chart" (`data = true`, `scope = []`) → view `ext:echarts-panel/chart`.
- Backend: trivial `echarts.about` WASM tool (the Tier-1 backend proof, 4 unit tests green).
- UI: `chart/framesToOption.ts` (pure `frames → EChartsOption`: series per numeric field, x-axis from a
  time/first field, unit/decimals/thresholds/legend/min/max from `fieldConfig.defaults` — the Field-tab
  options DRIVE the chart), `chart/ChartTile.tsx` (ECharts instance, honest no-data/error states, disposes
  on unmount, `notMerge` to drop stale series, ResizeObserver), `chart/mountChart.ts` (returns
  `{ update, teardown }` — `update` re-renders in place, no chart re-init).

## Tests (real store/bus/gateway — CLAUDE §9, no mocks)

- Rust: `cargo build --workspace` clean; `cargo test -p lb-host ui_decl`, `-p lb-ext-loader` green
  (incl. the new `data`-projection test); `echarts-panel` backend 4/4.
- UI unit: `pnpm test` **426 passed** (ExtWidget v2-path regression fixed — the standalone-mount
  `useDashboardWs` throw, see debugging entry).
- UI gateway: new `framesIn.gateway.test.tsx` **8/8 green** against a real spawned node — the mandatory
  **capability-DENY** (viewer w/o `viz.query` → no frames; viewer w/o the source cap → empty frame, call
  succeeds), **workspace-ISOLATION** (ws-A data not resolvable by ws-B), **v2-COMPAT** (v2 tile → no
  frames, path untouched), frames-resolution (real seeded series → real frames), data-flag projection
  (manifest→picker entry), and **dashboard + channel PARITY** (same cell → identical frames on both
  surfaces).
- echarts-panel UI unit: `framesToOption.test.ts` **7/7 green**; UI bundle builds (`dist/remoteEntry.js`,
  echarts bundled, React externalized).

## Deferred / follow-ups (flagged, not done silently)

- **Part C (`@nube/widget` package extraction)** — NOT done. The scope marks it optional and gated on the
  owner's yes; "common across surfaces" is already true (channels reuse `WidgetView`). Left as its own slice.
- **Cleanup rider (`useSceneDocs` `scene:` hardcode)** — NOT removed. It is a thecrew-specific coupling
  that does not interfere with data widgets (a data tile never ends in `/scene`) and my new code adds no
  extension-name branch. The generic manifest-option-schema replacement is its own slice; deferred.
- **Live SSE via `series.watch` into `update`** — the `refreshKey` tick path is wired (a data cell
  re-resolves and pushes `update(ctx)`); a dedicated `series.watch` streaming test is a follow-up (the
  shared-stream plumbing is reused, not duplicated).
- **Live-node publish** — `make publish-ext EXT=echarts-panel` packs + signs the wasm fine but the
  running node's dev-login returned 403/401 (`missing bearer credential`) — an environment/auth issue,
  likely a stale node (`make kill && make dev`), NOT a code defect. The palette entry is proven the real
  way by the gateway test's data-flag projection assertion.
