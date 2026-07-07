# Data Studio

Data Studio (`/t/$ws/data-studio`) is the **multi-pane data workbench**: a dockable, tabbed, splittable
surface where a user opens many data sources AND many of the app's own pages as dock panes — side by
side — explores real data, and turns what they find into reusable library panels (manually or with AI).
It is the **panel factory**; dashboards consume its output. It is a composition of shipped substrate,
not a new data or render path.

The 10x refresh (2026-07-05) replaced the layout engine with **Dockview**, lets the studio open the
app's **own pages as panes** (Flows top, Rules bottom, builder beside — one saved arrangement), and
reworked the builder into a **query-first → visual-viz-gallery → options-on-demand** flow with an
honest seeded-demo-data preview.

## What exists

- **A dockable workbench (Dockview).** Built on `dockview-react` (MIT, React-first): N builder tabs +
  N view panes open simultaneously; drag to split, tab, dock, float (pop out to a window), maximize,
  close; double-click a tab to rename (a `window.prompt` for now — popout parity kept, OQ4 resolved).
  Theme via Dockview's `--dv-*` CSS custom properties aliased to the shell tokens under
  `.dockview-theme-lb` (dark/light parity automatic). Tab titles cap + ellipsize with full-title
  tooltips. Two rail tabs — **Sources** and **Library** — sit beside the dock.
- **The rail's Sources tab is a `CatalogExplorer` host** (the workspace system catalog). Browse
  datasources → local tables → columns, series, channels, insights as ONE tree with the catalog's
  honest per-section deny/loading/empty states; click → open a builder tab on the picked entry (the
  studio's `onSelect` mapping: datasource → `federation.query`, table/column → `store.query`, series
  → `series.read`, channel → `inbox.list`, insight → `insight.get`). Replaces the bare `SourcePicker`
  select.
- **Pages-as-panes — the "+ Open view" menu.** Lists the core surfaces — Flows, Rules, Data,
  Datasources, Ingest — plus "New panel". Each opens as a dock pane mounting the **REAL routed view
  component** (`FlowsView`, `RulesView`, …): same code path, same gateway, same caps — never a
  re-implementation. An `embedded` mode on `AppPage` suppresses the view's own full-width header
  inside a pane (the dock tab is the title bar); the standalone routes keep it. One pane per view
  kind in the first cut (the menu re-activates an open pane; pages weren't written to be
  multi-mounted — shared subscriptions are the later fix, not a blocker). Data Studio itself is
  deliberately absent (no recursive embedding).
- **One stacked, query-first builder (10x phase 3).** Picking a source (or **New panel**, or opening
  a **Library** panel) opens a single builder tab with a **compact toolbar** (inline title, Run, freeze
  / table-view / inspect, one Save split-button) and the **focused query editor**. NO preview, NO viz
  pills, NO options rail until rows exist:
  - *Stage 2 — rows returned:* a **viz gallery** replaces the text pill row — one thumbnail card per
    widget type, each a live mini-render of the caller's ACTUAL frames through the one `viz.query` /
    `WidgetView` path (no second renderer). All cards share the draft's sources / transformations /
    fieldConfig, so they hit the SAME `vizQueryKey` cache entry — ONE query, N cheap views (asserted
    in tests). Table / AI-widget / Template render as labeled cards (a Template thumbnail is noise —
    OQ3 resolved). Shape-gating mirrors `VizPicker`: a card the data can't honestly fill is disabled.
  - *Stage 3 — refine on demand:* Query / Plot / Transform / Panel options / Field / Overrides fold
    into one collapsed, searchable Options drawer. Power depth intact, default cost zero.
- **Demo data, honestly seeded (rule 9).** When a query returns zero rows AND the seeded SQLite
  `demo-buildings` datasource exists in the workspace, the empty preview offers **"Preview with demo
  data"** — REAL records through the REAL `federation.query` engine (`SELECT time, value FROM
  point_reading …`), same render path. Demo state is clearly badged and **auto-yields the moment the
  user's own query has rows** (an unbadged demo frame in a control surface would be a lie).
- **Save to the library.** Any builder tab's Save split-button caret → **"Save as library panel"**
  persists the built spec as a `panel:{id}` (`panel.save`) — immediately reusable on any dashboard
  (Add library panel → a ref cell) and renderable standalone at `/t/$ws/panel/{id}`. The primary Save
  writes to the in-memory tab; the menu item is gated on `mcp:panel.save:call` (the host re-checks
  regardless). The saved-as marker is a compact badge ("library `<slug>`").
- **Layout persists per user.** The whole arrangement — every tab + every pane + geometry + each
  builder draft — persists to a **member-owned SurrealDB record** (`ui_layout:[ws, user, "data-studio"]`)
  via the `layout.get` / `layout.set` host verbs, debounced on change. The record is **VERSIONED by
  engine** (`{engine:"dockview", model}`). A legacy flexlayout blob (no `engine` tag, from v2/v3) →
  default workbench + a **one-time "layout was reset" notice** (drafts inside old layouts are the
  accepted loss; the library holds anything saved). It is never localStorage (rule 4), and it is keyed
  to the token `sub` — a member can only ever read/write their own layout.

## Query builder — common across dialects

The Builder⇄Code SQL editor is **one component for every datasource kind** — the dialect is behind
a seam, never a fork. A LOCAL TABLE source (SurrealDB, `store.query`) and an external DATASOURCE
(`federation.query` — postgres / timescale / sqlite, e.g. `demo-buildings`) get the **same** visual
builder (Table → Column/Aggregation → Filter → Group by → Order by → Limit + a live SQL preview)
and the **same** Code escape hatch. Lifting the deferral recorded in the v3 datasource-binding scope
("the federation schema-dropdown verb is deferred this phase") became possible once
`federation.schema {source, table?}` shipped; this slice is the lift.

- **One structured-query state, N dialect emitters.** The shared `SqlBuilderQuery`
  (`panel-kit/sql/query.ts`) is unchanged; the new seam is `emitSql(dialect, query)`
  (`panel-kit/sql/dialect.ts`). `toSurrealQL.ts` stays the SurrealQL impl (`math::sum(col)`, bare
  identifiers, `count()`); `toStandardSql.ts` is the standard-SQL impl (`SUM("col")`, double-quoted
  identifiers, `COUNT(*)`). The `dialect` is selected from the target's datasource `kind`
  (`surreal` vs `federation`) — config data, never a hardcoded datasource name (rule 10).
- **Federation dropdowns from `federation.schema`.** The visual builder's Table/Column dropdowns for
  a federation target are populated by the SHIPPED `federation.schema {source, table?}` verb —
  reusing the existing `discoverTables`/`describeTable` client, the same load pattern the editor
  uses for local `store.schema`. Tables load once per source; columns lazy-fill per picked table.
  No second schema-fetch path.
- **The Code editor is the escape hatch both ways.** Builder → Code is free (regenerate the string);
  Code → Builder confirms (hand-edited SQL may not round-trip — the same gate surreal had all along).
  A saved federation cell now carries `options.sql` (the builder state) so reopening returns to the
  builder — the round-trip surreal already had. **Migration:** a federation cell authored BEFORE this
  slice (no `options.sql`) reopens to Code mode with the saved SQL preserved verbatim — no fabricated
  builder query from hand-edited SQL.
- **Nothing else moves.** The wire shape (`federation.query {source, sql}` args) is unchanged. The
  render path (`viz.query`) is unchanged. No new MCP verb / cap / table / outbox target / host
  change — the slice is pure UI + one TS emitter module. Surreal-path behaviour is preserved
  byte-for-byte (pinned by the surreal-regression gateway test: dialect `math::avg` preview, not
  `AVG("…")`).

## The shared substrate

Data Studio's builder views are built on a **headless logic lib**, `ui/src/lib/panel-kit/`, extracted so
any surface can author panels with its own views (logic and views strictly separated):

- **Logic (headless, no JSX):** `cellToEditorState`/`editorStateToCell` (the ONE panel-spec
  (de)serializer), `usePanelEditor` (the editing state machine), `defaultCell`, the SQL builder model +
  `toSurrealQL`, `draftFromSelection`, `saveDraftAsPanel`, and `useGenUiAuthor` (the AI authoring hook).
- **Views:** `features/panel-builder/` (the option-surface tabs + the inline `BuilderPane`, now with a
  `stacked` query-first layout AND the split layout the dashboard parity tests use) and
  `features/data-studio/` (the Dockview panes + the rail). A third consumer can reuse the panel-kit
  logic with 100%-different views.

The genuinely-shared primitives are reused, not forked: `viz.query`/`usePanelData` (the one query path),
`WidgetHost`/`views/*` (rendering), `@nube/source-picker` (the catalog explorer skin the rail hosts),
the `panel.*` library asset, and the GenUI authoring seam.

## Panel authoring moved off the dashboard

The dashboard no longer authors panels. It **places** library panels (Add library panel → a ref cell)
and **renders** them; the Add-panel builder and the per-cell edit affordance were removed. To edit a
panel, open it in Data Studio's Library pane and save it back — one place authors panels now.

## How it fits the core

- **Capabilities (rule 5):** the surface shows for a member who can read data (`series.list`); every
  read re-checks its source tool's own cap (the `viz.query` per-target leash); Save-as-library needs
  `mcp:panel.save:call` (the split-menu caret is absent without it — the host re-checks regardless);
  layout persistence needs `mcp:layout.get:call` / `mcp:layout.set:call` (member-level); each embedded
  view pane re-checks its own verbs under the caller exactly as the routed page does. Deny paths
  degrade honestly — a denied source renders the standard `usePanelData` denied state; no
  `panel.save` → no Save-as-library affordance.
- **Tenancy / isolation (rule 6):** the built panel is a workspace-scoped `panel:{id}`; the layout
  record is walled by workspace AND keyed to the user; the seeded demo datasource is workspace-scoped
  like any registered source. Nothing crosses.
- **Core knows no extension (rule 10):** the picker lists extension tools/datasources as opaque
  entries; the `layout.*` `surface` key is opaque data; the pages-as-panes registry treats the pane
  kind as opaque data — the dock adapter (`ViewDockPanel`) looks up `viewPane(params.kind)` and renders
  `def.Component`, NEVER branching on a host subsystem id. Extension pages join the "+ Open view" menu
  later via the generic `ext.list` discovery — no extension id appears in the registry today.
- **MCP surface:** consumes the existing explore/render/library verbs; the `layout.get` / `layout.set`
  pair (the `nav_pref` pattern generalized) is the one substrate addition (shipped v2). No new verb /
  cap / table for the 10x refresh.

## Tests (real store/bus/gateway/caps, rule 9)

- **`DataStudio.gateway.test.tsx` (8 cases)** — pick source → stacked builder → save as library panel
  round-trip; member-owned layout; workspace isolation; capability-deny (no `panel.save`); SQL editor
  conditional; opening an existing library panel; rail minimize; **legacy-layout fallback** (a stored
  flexlayout blob → default workbench + reset notice).
- **`DataStudioPanes.gateway.test.tsx` (5 cases)** — open Flows + Rules panes render their REAL views;
  layout round-trip restores both; **AppPage embedded mode** (no header in-pane, header intact on the
  route); one pane per kind (re-activates); surfaces absent from `allowed` omitted from the menu.
- **`DataStudioBuilderFlow.gateway.test.tsx` (5 cases)** — query-first (no options pre-rows); **gallery
  renders N type cards from ONE real `viz.query`** (assert: one `mcp_call{viz.query}` for preview + 9
  thumbnails — React Query de-dups, the view is not in the key); `panel.save` round-trip; demo-data
  integrity (rows ⇒ no offer, no badge); capability-deny (`viz.query` absent ⇒ honest degrade).
- **Units (33 cases across 5 files)** — record versioning (`workbenchModel.test.ts`), pane registry
  (`workbenchPanes.test.ts`), gallery type-mapping + shape-gating (`VizGallery.test.tsx`), drawer
  disclosure + search (`OptionsDrawer.test.tsx`), demo state machine + auto-yield
  (`useDemoPreview.test.ts`).
- `panelEditor.gateway` + `flowsPanelEditor.gateway` (split layout parity) **10/10** green — phase 3's
  stacked path left the split path untouched.

## Not yet

- `@nube/panel-kit` as a standalone `packages/*` package (the logic is package-shaped; the type-graph
  extraction is a follow-up).
- Shared/team layouts and named layout presets (one layout per user per surface today).
- A conversational data-QA agent (the GenUI authoring tab + the catalog explorer are the AI paths
  today).
- Extension pages in the "+ Open view" menu via `ext.list` discovery (the registry excludes them
  today — rule 10).
- Sub-pane granularity for pages-as-panes (whole pages only — OQ1 resolved as "whole pages first";
  sub-panes only if the whole-page pane proves too coarse in use).
- Shared subscriptions for multi-mounted pages (today one pane per kind; two Flows panes would
  double the polling/SSE — the menu disables an open kind).
