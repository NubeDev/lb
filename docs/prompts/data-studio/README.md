# Data Studio — orientation for a fresh session

Paste this at the top of a new AI session working on Data Studio. It's a map, not a
spec — it tells you *what exists and where*, so you don't re-explore the tree. Read the
linked scope/session docs for the *why*; read the files for the *how*.

> **Golden rules for this surface** (from `CLAUDE.md`): rule 9 — no mocks/fakes, drive the
> **real** gateway/store; rule 8 — one responsibility per file (≤400 lines); rule 10 — no
> branching on extension ids. The studio is a **composition** surface: it reuses the shipped
> `viz.query` render path, `@nube/source-picker`, panel-kit, and `panel.*` library — it does
> **not** add a render/query substrate.

## What it is

`/t/$ws/data-studio` — a **Dockview** multi-pane workbench. Left rail (`StudioRail`:
Sources / Library) + a dock of N tabs you split/tab/float/maximize/pop-out. Two things you
open into the dock:

1. **Builder tabs** — the query-first panel builder (query → viz gallery → options drawer),
   saved to the library as a `panel:{id}`.
2. **Pages-as-panes** — the app's OWN routed views (Flows, Rules, Data, Datasources, Ingest)
   mounted as dock panes via "+ Open view", so you debug across surfaces in one saved layout.

The whole arrangement (panes + geometry + every builder draft) persists **per member** in
SurrealDB via `layout.get`/`layout.set` (record `ui_layout:[ws, user, "data-studio"]`).

History: v1 single-preview → v2 FlexLayout multi-pane → v3 stacked query/preview builder →
**10x** (Dockview + pages-as-panes + query-first builder + SQLite demo data, shipped
2026-07-05). The 10x rebuild is the current shape.

## Where the code lives

### `ui/src/features/data-studio/`
| File | Purpose |
|---|---|
| `DataStudioView.tsx` | The workbench: `StudioRail` + the Dockview dock; the entry point. |
| `index.ts` | Surface export (mounted by `routing/createAppRouter.tsx` → `/data-studio`). |
| `workbenchModel.ts` | Dockview model vocabulary — pane kinds, per-pane `params`, the **versioned** persisted record (`{engine:"dockview", model}`), id mint. Pure data. |
| `workbenchPanes.tsx` | **Pages-as-panes registry** — view kind → the REAL routed view component (`FlowsView`, `RulesView`, …), gated by each surface's `allowed` lens. Rule-10-safe: ids are data. |
| `workbenchContext.ts` | Host context every dock panel needs (ws, var scope, library-refresh cb, layout `touch`). |
| `useWorkbenchLayout.ts` | Load/persist seam — `layout.get`/`set`, debounced; legacy flexlayout blobs fall back to default + reset notice. |
| `OpenViewMenu.tsx` | The "+ Open view" header menu (New panel + the pages, filtered by route gating). |
| `WorkbenchTab.tsx` | Dock tab renderer — title cap/ellipsis + tooltip, dbl-click rename, close. |
| `dockviewTheme.ts` / `datastudio-dock.css` | Dockview theme bridge → shadcn tokens. |
| `StudioRail.tsx` | Left rail on shared `AppRail`/RosterRail chrome. Tabs: Sources / Library. |
| `panes/BuilderDockPanel.tsx` | Dockview wrapper → `BuilderTabPane`; stows drafts, `touch()`es layout. |
| `panes/ViewDockPanel.tsx` | Dockview wrapper mounting a registry view in embedded-page mode. |
| `panes/BuilderTabPane.tsx` | One builder tab: the stacked `BuilderPane` over headless panel-kit. |
| `panes/SourcesPane.tsx` | Sources rail tab — a `CatalogExplorer` host (system catalog). |
| `panes/LibraryPane.tsx` | Library rail tab — `panel.list` roster, opens each in a builder tab. |

### `ui/src/features/panel-builder/` — the query-first builder (shared with dashboards)
| File | Purpose |
|---|---|
| `BuilderPane.tsx` | The builder VIEW over headless `usePanelEditor`; `layout="stacked"` (studio, query-first) vs `"split"` (dashboard editor). |
| `BuilderToolbar.tsx` | Compact toolbar — title, Run, freeze/table/inspect, Save split-button. |
| `PreviewPane.tsx` | Live preview via the SAME `WidgetView` + `usePanelData` that save uses. |
| `VizGallery.tsx` | Thumbnail cards — one live mini-render per widget type from the ONE fetched query (view not in the query key → N cheap views, one query). |
| `VizPicker.tsx` | Flow-source viz picker (shape-gated). |
| `OptionsDrawer.tsx` | The options disclosure — **open by default** (editing must never be hidden). |
| `OptionsSections.tsx` | The option surface — Query/Plot/Transform/Panel options/Field/Overrides + search. |
| `OptionsSearch.tsx` | Options search input. |
| `useDemoPreview.ts` | Zero-rows → "Preview with demo data": display-only swap to the SQLite `demo-buildings` datasource. |
| `LibraryPanelBar.tsx` | Save-as-library / used-on-N / unlink over `panel.*`. |

### `packages/source-picker/src/` (`@nube/source-picker`) — the workspace system catalog
The rail's Sources tab is a **`CatalogExplorer`** tree. Model + loaders + two skins (the
`<select>`/combobox picker AND the tree). **After ANY change here run `pnpm build` in the
package — the app imports `dist/`** (a stale dist is a classic "does not provide an export"
crash).
- `catalog.ts` / `loadCatalog.ts` / `useCatalog.ts` — the catalog MODEL (sections as data) + loader + hook.
- `CatalogExplorer.tsx` / `CatalogSection.tsx` / `CatalogSchemaTree.tsx` — the tree skin.
- `SourcePicker.tsx` / `SourceCombobox.tsx` / `sourcePicker.ts` / `useSourcePicker.ts` — the picker skin (a projection off the catalog).
- `types.ts` — the injected `SourceLoaders` seam (transport-agnostic); `index.ts` — public surface.

### Rust — layout persistence + datasources
- `rust/crates/host/src/layout/` — the ui-layout service (member-owned `ui_layout:[ws,user,surface]`): `mod.rs`, `get.rs`, `set.rs`, `model.rs`, `store.rs`, `tool.rs`, `error.rs`.
- `rust/role/gateway/src/routes/layout.rs` — `layout.*` browser routes (re-run `mcp:layout.<verb>:call`).
- `rust/role/gateway/src/routes/datasources.rs` — `datasource.*` admin routes.

### SQLite demo datasource (answers 10x demo-data the lite way)
- `rust/extensions/federation/src/source/{mod,sqlite,postgres}.rs` — the `Source` trait (one external SQL engine per impl file); `sqlite.rs` is a REAL on-disk engine, refuses a missing path.
- `docker/postgres/seed.py --sqlite <path>` — same building dataset into one `.db` (lite profile: `--months 1 --interval 15`).
- `docker/postgres/seed-demo-sqlite.sh` + `make seed-demo-sqlite` — generate `.lazybones/data/demo/buildings.db` and register `demo-buildings` via `datasource.add`.
- `ui/src/features/datasources/AddDatasourceForm.tsx` — kind `<select>` over the `KINDS` data array (postgres/timescale/sqlite; sqlite DSN = file path).

## Docs (read for the *why*)
- **Scope:** `docs/scope/frontend/data-studio-10x-scope.md` (SHIPPED — the current shape) ·
  `docs/scope/frontend/data-studio-scope.md` (v2/v3) ·
  `docs/scope/frontend/system-catalog-scope.md` (the catalog tree, ask) ·
  `docs/scope/datasources/sqlite-datasource-demo-scope.md` (SHIPPED).
- **Query builder 10x (Tabularis-grade + a standalone Query view that mounts here as a pane):**
  `docs/scope/frontend/query-builder/` — start with `query-builder-10x-scope.md` (umbrella); slice 3
  (`query-workbench-view-scope.md`) is the new `+ Open view → Query` pane — it follows the `workbenchPanes.tsx`
  pages-as-panes registry + `OpenViewMenu.tsx` documented in the tables above (one `VIEW_PANES` line).
- **Public:** `doc-site/content/public/frontend/data-studio.mdx`.
- **Sessions (the messy middle):** `docs/sessions/frontend/data-studio-10x-session.md`,
  `data-studio-rail-session.md`, `data-studio-v2-workbench-session.md`,
  `data-studio-v3-stacked-view-session.md`, `dashboard-open-in-data-studio-session.md`.
- **Bug history:** `docs/debugging/frontend/` (grep `data-studio`).
- **Dashboards** (the sibling surface, shares panel-builder): `docs/prompts/dashboard/README.md`.

## Build & test
```bash
cd ui && pnpm test                                        # unit (vitest)
cd ui && pnpm test:gateway <file>                         # REAL spawned gateway (rule 9)
cd ui && pnpm tsc --noEmit                                # (pre-existing errors: FlowsCanvas.gateway, transformDebug.gateway — ignore)
cd packages/source-picker && pnpm build                   # after ANY source-picker change — app imports dist/
make seed-demo-sqlite                                     # register the Docker-free demo datasource on a running node
```
Gateway tests: `ui/src/features/data-studio/{DataStudio,DataStudioBuilderFlow,DataStudioPanes}.gateway.test.tsx`
(+ unit `workbenchModel.test.ts`, `workbenchPanes.test.ts`). They rect-stub the DOM because
Dockview measures layout in jsdom — copy that pattern for a new gateway test.

## Gotchas
- **Chart escaping its container** — a Recharts chart needs a *definite* height; inside an
  `overflow-y-auto` column it inflates unbounded and buries the controls. Preview/gallery/
  drawer columns are deliberately **bounded**, not scrolling. (`SeriesLineChart` in
  `ui/src/features/dashboard/widgets/recharts.tsx` uses `ResponsiveContainer`.)
- **Stale package dist** — see the source-picker note above.
- **Options must stay reachable** — `OptionsDrawer` defaults open; never gate the *affordance*
  behind rows, only the *content*.
