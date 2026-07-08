# Dashboard — orientation for a fresh session

Paste this at the top of a new AI session working on Dashboards. It's a map, not a spec —
it tells you *what exists and where*, so you don't re-explore the tree. Read the linked
scope/session docs for the *why*; read the files for the *how*.

> **Golden rules for this surface** (from `CLAUDE.md`): rule 9 — no mocks/fakes, drive the
> **real** gateway/store, seed real records; rule 8 — one responsibility per file (≤400 lines);
> rule 4 — layout is a SurrealDB record, **never localStorage**; rule 5 — the host is the only
> capability boundary. Editing is **admin-only** (viewer-mode); a member sees but can't author.

## What it is

`/t/$ws/dashboards` — a grid of widgets over real data. A dashboard is an **asset**: a
workspace-namespaced `dashboard:{id}` SurrealDB record holding a grid layout (`cells[]`),
read through the **three-gate check** (workspace → capability → membership/visibility). Layout
edits (add/remove/drag/resize a cell) persist via `dashboard.save`. Widgets render through the
**one** `viz.query` path; a saved chart can be promoted to a reusable **library panel**
(`panel:{id}`), which Data Studio authors and the dashboard just places.

The panel builder/editor itself is **shared with Data Studio** — it lives in
`ui/src/features/panel-builder/` and `ui/src/lib/panel-kit/`. The dashboard uses it in
`layout="split"`; Data Studio uses `layout="stacked"`. See `docs/prompts/data-studio/README.md`.

## Where the code lives

### `ui/src/features/dashboard/` — the surface
| File / dir | Purpose |
|---|---|
| `DashboardView.tsx` | The surface: roster + grid; layout edits → `dashboard.save`. Entry point (routed at `/dashboards`). |
| `index.ts` | Surface export. |
| `Grid.tsx` | The react-grid-layout grid over `cells[]`; drag/resize stops persist geometry via `onLayout`. |
| `WidgetHost.tsx` | Per-cell dispatch: a v3 `view`+`sources[]` cell → `WidgetView`; a v1/v2 cell → the built-in chart/stat/gauge over `useSeries`. |
| `WidgetPlaceholder.tsx` | Empty/loading cell chrome. |
| `DashboardRoster.tsx` | The left roster (admin-only authoring surface); on shared RosterRail kit. `.test.tsx` beside it. |
| `AddLibraryPanel.tsx` | Place an existing `panel:{id}` (from `panel.list`) as a ref cell — spec hydrated host-side on `dashboard.get`. |
| `RefreshControl.tsx` / `useAutoRefresh.ts` | `?refresh=30s` auto-refresh — bumps a `refreshKey` re-running each cell's read source. |
| `useDashboard.ts` | Loads + mutates the dashboard record; all writes go through `dashboard.save`. |
| `useSeries.ts` | The v1/v2 built-in widget data hook (`series.read`/`series.latest`). |
| `builder/` | The v3 data/render plumbing: `usePanelData.ts` (the ONE `viz.query` hook, invariant A), `useVizQuery/useVizFrames/useVizSteps.ts`, `ExtWidget.tsx` + `WidgetIframe.tsx` (extension `[[widget]]` host over the mediated bridge), `sql/` (Builder⇄Code SQL editor), `TemplateView`/`sanitizeTemplateHtml`. |
| `views/` | The render vocabulary — one folder per viz: `timeseries/`, `barchart/`, `bargauge/`, `gauge/`, `piechart/`, `stat/`, `table/`, `genui/`; `WidgetView.tsx` is the dispatch seam; `shape.ts` gates which views a result shape can fill; `reduce.ts`/`field.ts`/`plot.ts` the frame→value math. |
| `widgets/` | The v1/v2 built-ins: `ChartWidget/StatWidget/GaugeWidget.tsx`, `recharts.tsx` (shared chart SVGs — **`SeriesLineChart` uses `ResponsiveContainer`; a chart needs a definite-height host or it inflates**), `chrome.tsx`. |
| `fieldconfig/` | The Grafana fieldConfig bridge — `format.ts` (the ONE user-prefs number/date/unit formatter — never a local `toFixed`), `color.ts`, `thresholds.ts`, `mappings.ts`, `matchers.ts`, `units.ts`, `resolve.ts`, `useFormattedValue.ts`. |
| `vars/` | Dashboard variables — `useVarScope.ts`, `VariableBar.tsx`, `VariableEditor.tsx`, `RequiredVarGate.tsx`, `resolveOptions.ts`. |
| `cache/` | The per-visit read cache (react-query, scoped to the route): `DashboardQueryProvider.tsx`, `queryKeys.ts`, `useFreeze.ts` (edit-without-requery), `dashboardQueryClient.ts`. |

### Shared panel builder (dashboards + Data Studio)
- `ui/src/features/panel-builder/` — `BuilderPane.tsx` (`layout="split"` here), toolbar, preview, viz gallery/picker, options sections/drawer, `LibraryPanelBar.tsx`.
- `ui/src/lib/panel-kit/` — the headless machine: `usePanelEditor.ts`, `cellEditorState.ts` (the pinned `editorStateToCell(cellToEditorState(c))≡c` (de)serializer), `defaultCell.ts`, `draftFromSelection.ts`, `saveAsLibrary.ts`, `useGenUiAuthor.ts`, `sql/`.

### Lib types & clients
- `ui/src/lib/dashboard/` — `dashboard.types.ts` (`Cell`, `Dashboard`, schemaVersion 3), `dashboard.api.ts`, `cellView.ts`, `fieldconfig.types.ts`, `series.stream.ts`/`bus.stream.ts`.
- `ui/src/lib/panel/` — `panel.types.ts`, `panel.api.ts` (`listPanels/getPanel/savePanel/deletePanel/sharePanel/panelUsage`), `panel.cell.ts` (`specToCell`).

### Rust — the host chokepoint + gateway
- `rust/crates/host/src/dashboard/` — the dashboard **service** (asset model, three-gate). One verb per file: `get.rs`, `list.rs`, `save.rs`, `delete.rs`, `share.rs`, `pin.rs` (pin a tool result to a dashboard), `seed.rs`; plus `authorize.rs`, `visibility.rs`, `bounds.rs` (record caps: ≤32 transforms etc.), `catalog.rs`/`genui.rs` (+ `widget_catalog.json`/`genui_catalog.json`), `views.rs`, `model.rs`, `store.rs`, `tool.rs`, `error.rs`.
- `rust/role/gateway/src/routes/dashboard.rs` — the `dashboard.*` browser routes (re-run `mcp:dashboard.<verb>:call`).

## Docs (read for the *why*)
- **Scope (flat, older):** `docs/scope/frontend/dashboard-scope.md` (the surface + asset/authz model) · `dashboard-viewer-mode-scope.md` (admin-only editing) · `dashboard-widgets-scope.md` (widgets as extensions, the mediated bridge) · `dashboard-query-cache-scope.md`.
- **Scope (subtopic — new notes go here):** `docs/scope/frontend/dashboard/README.md` is the index. Key ones: `library-panels-scope.md`, `source-picker-package-scope.md`, `rules-as-source-scope.md`, `ext-widget-source-binding-scope.md`, `reusable-pages-scope.md`, `data-studio-ux-scope.md`, the `widget-*` set. **`dashboard/viz/`** holds the Grafana-compat visualization slice: `panel-model-scope.md`, `panel-editor-scope.md`, `chart-types-scope.md`, `field-config-scope.md`, `transformations-scope.md`, `datasource-binding-scope.md`, `import-export-scope.md`, `editor-parity-scope.md`, `xy-plot-builder-scope.md`.
- **Public:** `doc-site/content/public/frontend/dashboard.mdx` (the trimmed shipped truth — large; grep it).
- **Sessions (the messy middle, ~30 files):** `docs/sessions/frontend/` — grep `dashboard`, `widget`, `viz`, `panel`, `xy-plot`.
- **Bug history:** `docs/debugging/frontend/` (grep `dashboard`/`viz`/`widget`).
- **Data Studio** (sibling surface, shares the builder): `docs/prompts/data-studio/README.md`.

## Build & test
```bash
cd ui && pnpm test                            # unit (vitest)
cd ui && pnpm test:gateway <file>             # REAL spawned gateway (rule 9)
cd ui && pnpm tsc --noEmit                    # (pre-existing errors: FlowsCanvas.gateway, transformDebug.gateway — ignore)
cd rust && cargo test --workspace             # host dashboard_test etc. (see ext-wasm prereq in memory)
```
Gateway tests: `ui/src/features/dashboard/{DashboardView,ReusablePages}.gateway.test.tsx`
(+ many `*.gateway.test.tsx` under `builder/` and `views/`). Rust: `lb-host dashboard_test`,
gateway `dashboard_routes_test`.

## Gotchas / invariants
- **One query path** — all v3 data comes through `usePanelData` → `viz.query` (invariant A). No parallel renderer, no client-side transform pipeline (invariant B — transforms are backend-resolved).
- **The (de)serializer identity** — `editorStateToCell(cellToEditorState(c)) ≡ c` for v1/v2/v3 cells. Break it and "edit loses my SQL options" / "add ≠ edit" bugs return.
- **Formatting goes through `fieldconfig/format.ts`** — never a local `toFixed`/unit string; it's the one user-prefs bridge.
- **Charts need a definite-height host** — `recharts.tsx`'s `SeriesLineChart` uses `ResponsiveContainer`; a scrolling/auto-height parent makes it inflate unbounded.
- **Editing is admin-only** — `DashboardView` mounts the roster + edit affordances behind `isAdmin(caps)`; the host re-checks owner + cap regardless (never trust the UI gate).
