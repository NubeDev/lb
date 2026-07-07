# Frontend — Data Studio 10x: Dockview workbench, pages-as-panes, query-first builder (session)

- Date: 2026-07-05
- Scope: ../../scope/frontend/data-studio-10x-scope.md (all 4 phases — the CODE-ONLY first session
  shipped the implementation; this session wrote the tests, fixed the bugs they surfaced, and wrote
  the docs)
- Stage: post-S8 (data plane shipped) — a UI slice over shipped layout / panel / viz.query / page
  substrate (no new verb / cap / table)
- Status: done

## Goal

Finish the data-studio-10x scope: write the testing plan against the REAL gateway, fix any bugs the
tests surface, and write the docs. The prior CODE-ONLY session shipped all 4 phases (engine swap →
pages-as-panes → query-first builder + demo-data → CatalogExplorer rail). This session owns the
mandatory categories (capability-deny per gated verb, workspace-isolation), the phase-by-phase cases
(legacy-layout fallback, REAL view mount inside a pane, ONE-query gallery, demo integrity), the unit
suite (record versioning, pane registry, gallery type-mapping, drawer disclosure, demo state machine),
and the docs/debug history promotion.

## What changed (this session)

**Tests written (no production code in this session beyond the radius-guard fix below):**

- `ui/src/features/data-studio/DataStudio.gateway.test.tsx` (rewritten, 8 cases) — adapted the
  existing v2/v3 suite to the CatalogExplorer picker (click `insert series cooler.temp`), the Save
  split-button caret ("more save options" → "save as library panel"), and the stacked builder's
  Run-first staging (preview/gallery/options hidden until rows exist). Adds the legacy-layout
  fallback case (a stored flexlayout blob → default workbench + the one-time reset notice, no crash).
- `ui/src/features/data-studio/DataStudioPanes.gateway.test.tsx` (NEW, 5 cases) — phase 2: open Flows
  + Rules panes via the "+ Open view" menu (both render their REAL routed views against the gateway);
  layout round-trip restores both panes by their deterministic `view:<kind>` ids; AppPage embedded
  mode (no full-width `<h1>` in-pane; the standalone route keeps its header); one pane per kind
  (re-activates instead of duplicating); and the menu omits surfaces absent from the caller's
  `allowed` route lens (UI gate; the gateway re-checks every verb regardless).
- `ui/src/features/data-studio/DataStudioBuilderFlow.gateway.test.tsx` (NEW, 5 cases) — phase 3: the
  query-first flow (pre-Run stage 1 only; post-Run reveals preview + gallery + drawer); the ONE-query
  gallery (preview + 9 thumbnail cards render from exactly ONE `mcp_call{viz.query}` — React Query
  de-dups, the `view` is not part of the cache key); `panel.save` round-trip through the split-menu;
  demo-data integrity (a row-full query ⇒ no demo offer AND no demo badge); and the capability-deny
  (without `viz.query`, the preview degrades honestly — never a fabricated value).
- `ui/src/features/data-studio/workbenchModel.test.ts` (NEW, 10 cases) — the versioned record seam
  (`loadWorkbench`): null/undefined → default silently; tagged dockview → restore verbatim; a legacy
  flexlayout blob (no `engine` tag) → default + reset notice; a tagged-but-null model is treated as
  non-migratable (forward-safe); a foreign engine tag (a future `dockview2`) → default + notice.
- `ui/src/features/data-studio/workbenchPanes.test.ts` (NEW, 5 cases) — the registry lists the 5
  core surfaces (flows/rules/data/datasources/ingest), excludes the host surface (no recursive
  embedding), kinds are opaque data (rule 10 — the dock never branches on a host subsystem id).
- `ui/src/features/panel-builder/VizGallery.test.tsx` (NEW, 6 cases) — the type-mapping (6 chart-like
  thumbnails + 3 labeled cards = 9 type cards), the selected card is `aria-pressed`, shape-gating
  disables a card the data can't honestly fill (parity with `VizPicker`), clicking an enabled card
  fires `onChange`, clicking a disabled card is a no-op, the `unknown` shape leaves every card
  enabled (pre-data; permissive).
- `ui/src/features/panel-builder/OptionsDrawer.test.tsx` (NEW, 4 cases) — collapsed by default (zero
  cost), expands on click (aria-expanded flips, children mount), collapses on a second click, the
  searchable OptionsSections surface (the `search options` input) only mounts on expand.
- `ui/src/features/panel-builder/useDemoPreview.test.ts` (NEW, 8 cases) — the demo state machine the
  gateway suite can't reach cleanly (driving a real 0-row query through CodeMirror's `getClientRects`
  jsdom gap is the pre-existing `transformDebug.gateway` red). Asserts: `available` is gated on the
  demo datasource existing in the roster, a target being staged, the query being idle, AND zero rows;
  `enable()` flips `active`; the hook AUTO-YIELDS the moment the user's own query has rows (the
  correctness requirement — an unbadged demo frame in a control surface is a lie); `disable()` turns
  it off; `demoSwappedCell` swaps the data binding to `federation.query` over the demo dataset while
  preserving the user's view/title/fieldConfig.

**Production fix surfaced by the new tests:**

- `ui/src/styles/radius-scale.guard.test.ts` red — the CODE-ONLY session's new files used bare
  `rounded` (banned by the radius guard shipped 2026-07-04). Six offenders across
  `OpenViewMenu.tsx` (×2), `WorkbenchTab.tsx`, `panes/BuilderTabPane.tsx`, `BuilderPane.tsx` (demo
  badge), `BuilderToolbar.tsx`. All six fixed to token-derived stops (`rounded-md` for menu items,
  `rounded-sm` for tight chips/icon buttons). Debug entry:
  [../../debugging/frontend/data-studio-10x-bare-rounded-radius-guard.md](../../debugging/frontend/data-studio-10x-bare-rounded-radius-guard.md).

## Decisions & alternatives

- **Drive the demo hook's 0-row path via a UNIT test, not the gateway suite.** The honest end-to-end
  demo-offer-then-toggle path needs a user query that returns 0 rows. Constructing one through the UI
  means typing SQL into CodeMirror, which throws `textRange(...).getClientRects is not a function`
  inside jsdom's animation-frame measure (the same pre-existing gap that keeps
  `transformDebug.gateway` red on clean master). Decision: the gateway suite proves the demo
  *integrity* (rows ⇒ no offer, no badge — the honest-integration half) plus the demo datasource
  really existing via `addDatasource`; the demo *state machine* (offer / enable / auto-yield /
  disable) is unit-tested against the hook with the ONE pure-data seam it reads (`useDatasourceList`)
  observing a fake-loader object — the system-catalog precedent for "fake a pure function seam, never
  a backend" (rule 9 / testing-scope §0).
- **Inactive tabs assert via the tab strip, not the rendered pane.** Dockview, like FlexLayout,
  unmounts an inactive tab's content (only the active panel keeps its React tree). The "two panes
  coexist" assertion therefore reads the dock's tab strip (each tab's `title` attribute carries the
  pane name verbatim), not a simultaneous `getByLabelText("flows view")` + `getByLabelText("rules
  workbench")`. Recorded here so the next person extending the panes suite doesn't chase a phantom
  "the inactive pane disappeared" bug.
- **`fireEvent.click` for the open-view menu, not `userEvent.click`.** `userEvent`'s pointer-event
  sequence races the menu's `mousedown` light-dismiss handler — the menu closes before the item's
  `onClick` fires. `fireEvent.click` dispatches a synthetic `click` only, which the dismiss handler
  ignores. Same pattern the existing picker/combobox tests use.
- **Wait for the dock's empty-watermark before clicking open-view.** `bench.api` (the Dockview api
  handle `openView` reads) is null until `onReady` fires — a small asynchronous window AFTER
  `bench.ready=true` (the dock mounts when ready flips, then onReady runs on the next tick). Without
  the watermark wait, the first `openView` is a silent no-op. The watermark text is the right
  readiness signal because it renders with the dock itself.
- **Scoped `Range.getClientRects` polyfill in `DataStudioPanes.gateway.test.tsx`.** The Rules pane
  mounts a CodeMirror editor, which measures glyph bounds via `Range.getClientRects()` — unimplemented
  in jsdom. Without a scoped polyfill, the uncaught exception inside CodeMirror's animation-frame
  measure kills the dock's React tree. Scoped to this one test file (NOT promoted to
  `setup-gateway.ts`) so the pre-existing red (`transformDebug.gateway`) stays untouched per the
  session's scope.

## Tests — green output

Real store/bus/gateway/caps throughout (rule 9); no fake backends. The only fake is the pure
data-seam `useDatasourceList` in the demo-hook unit (the system-catalog precedent).

**UI gateway suite (3 files / 18 cases) — `pnpm test:gateway src/features/data-studio/`:**
```
 ✓ src/features/data-studio/DataStudio.gateway.test.tsx (8 tests) 5813ms
   ✓ Data Studio 10x — the Dockview workbench (real gateway) > pick source → stacked builder → save as library panel round-trips; the layout + draft persist per user 2345ms
   ✓ Data Studio 10x — the Dockview workbench (real gateway) > the layout record is MEMBER-OWNED — another member gets their own default workbench 1019ms
   ✓ Data Studio 10x — the Dockview workbench (real gateway) > workspace isolation — the layout and the saved panel never cross to ws-B 1150ms
   ✓ Data Studio 10x — the Dockview workbench (real gateway) > capability-deny — no `panel.save`: no split-menu save-as-library, and the verb is refused server-side 1003ms
   ✓ Data Studio 10x — the Dockview workbench (real gateway) > SQL editor surfaces for a Direct-SurrealDB source and is absent for a series source (stacked) 544ms
   ✓ Data Studio 10x — the Dockview workbench (real gateway) > opening an existing library panel lands in the stacked builder (preview + query, one tab) 711ms
   ✓ Data Studio 10x — the Dockview workbench (real gateway) > the studio rail minimizes to the shared collapsed strip and expands back 391ms
   ✓ Data Studio 10x — the Dockview workbench (real gateway) > legacy-layout fallback — a stored flexlayout blob → default workbench + the one-time reset notice 649ms
 ✓ src/features/data-studio/DataStudioPanes.gateway.test.tsx (5 tests) 3363ms
   ✓ Data Studio 10x — pages-as-panes (real gateway) > opens Flows + Rules panes via '+ Open view' and both render their REAL routed views 564ms
   ✓ Data Studio 10x — pages-as-panes (real gateway) > the pane arrangement round-trips through layout.set — a reload restores both panes 1494ms
   ✓ Data Studio 10x — pages-as-panes (real gateway) > AppPage embedded mode: a view in a pane has NO full-width header; the standalone route keeps it 416ms
   ✓ Data Studio 10x — pages-as-panes (real gateway) > a view pane re-activates instead of duplicating (one pane per view kind) 660ms
   ✓ Data Studio 10x — pages-as-panes (real gateway) > capability-deny — a surface absent from `allowed` is omitted from the open-view menu 362ms
 ✓ src/features/data-studio/DataStudioBuilderFlow.gateway.test.tsx (5 tests) 2806ms
   ✓ Data Studio 10x — phase 3 query-first builder (real gateway) > query-first: pre-Run mounts ONLY the toolbar + query editor; rows reveal preview + gallery + drawer 570ms
   ✓ Data Studio 10x — phase 3 query-first builder (real gateway) > the gallery renders N type cards from ONE real viz.query — preview + thumbnails share the cache 472ms
   ✓ Data Studio 10x — phase 3 query-first builder (real gateway) > panel.save round-trip is unchanged — the stacked builder's split-menu Save-as-library writes a real record 1102ms
   ✓ Data Studio 10x — phase 3 query-first builder (real gateway) > demo-data integrity: when the user query has rows, no demo offer / no demo badge 477ms
   ✓ Data Studio 10x — phase 3 query-first builder (real gateway) > capability-deny — without the viz.query cap, the preview degrades honestly (no fabricated rows) 430ms

 Test Files  3 passed (3)
      Tests  18 passed (18)
```

**UI unit suite (5 new files / 33 cases) — `pnpm test`:**
```
 ✓ src/features/data-studio/workbenchModel.test.ts (10 tests) 2ms
 ✓ src/features/data-studio/workbenchPanes.test.ts (5 tests) 6ms
 ✓ src/features/panel-builder/VizGallery.test.tsx (6 tests) 69ms
 ✓ src/features/panel-builder/OptionsDrawer.test.tsx (4 tests) 30ms
 ✓ src/features/panel-builder/useDemoPreview.test.ts (8 tests) 13ms
```

Full repo UI unit green: **705/705** (was 672; +33 new — the 5 files above). `pnpm exec tsc --noEmit`
clean (only the two pre-existing reds remain: `FlowsCanvas.gateway`, `transformDebug.gateway`).
`panelEditor.gateway` + `flowsPanelEditor.gateway` (the split-layout parity gate) **10/10** green —
phase 3's stacked path left the split path untouched.

## Definition of done (HOW-TO-CODE §5)

- [x] Satisfies the scope and the stage's exit gate (4 phases built + tested).
- [x] Full API surface the scope named: consumes only (layout.get/set, viz.query, panel.*, each
  pane's own verbs); no new CRUD, no batch, no extension special-casing.
- [x] FILE-LAYOUT respected (one responsibility per file; tests mirror the source files).
- [x] No `if cloud {…}` — frontend-only, symmetric (browser + Tauri identical).
- [x] No core knowledge of any extension — the pane kinds are opaque data; `workbenchPanes.test.ts`
  pins this.
- [x] No mock data / no fake backend — `useDemoPreview`'s only fake is a pure function seam (the
  system-catalog precedent); the rest runs on real seeded rows through the real gateway.
- [x] Tests exist on both backend (N/A — frontend-only slice) and frontend, including the mandatory
  capability-deny (`panel.save` deny, `viz.query` deny, surface-allowed deny) and workspace-
  isolation (the layout + panel never cross ws-B), and a deny per gated verb; green output pasted.
- [x] Every bug fixed this session has a regression test (the radius guard IS the regression for the
  bare-`rounded` fix) and a closed debug entry.
- [x] `sessions/frontend/data-studio-10x-session.md` filled in (this file).
- [x] Anything shipped promoted to `public/frontend/data-studio.md` + the open questions refreshed.
- [x] `STATUS.md` reflects the new state.
- [x] scope ↔ session ↔ public ↔ debug cross-linked.

## Related

- Scope: [../../scope/frontend/data-studio-10x-scope.md](../../scope/frontend/data-studio-10x-scope.md)
- Public: [../../public/frontend/data-studio.md](../../public/frontend/data-studio.md)
- Debug: [../../debugging/frontend/data-studio-10x-bare-rounded-radius-guard.md](../../debugging/frontend/data-studio-10x-bare-rounded-radius-guard.md)
- Companion (shipped same family): the SQLite demo datasource that backs the demo-data toggle —
  [../../scope/datasources/sqlite-datasource-demo-scope.md](../../scope/datasources/sqlite-datasource-demo-scope.md)
  + [../../sessions/datasources/sqlite-datasource-demo-session.md](../../sessions/datasources/sqlite-datasource-demo-session.md).
