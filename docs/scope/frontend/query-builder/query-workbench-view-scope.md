# Query workbench view scope — a standalone surface (like Flows/Rules) that also opens as a Data Studio pane

Status: scope (the ask). Slice 3 of [`query-builder-10x-scope.md`](query-builder-10x-scope.md). Promotes
to `public/frontend/query-builder.md` on ship.

> **RETARGETED (peer review, 2026-07-06, user decision).** No new `/query` rail surface. The peer review
> found the **existing Datasources page already ships half of this slice**:
> `features/datasources/DatasourceDetail.tsx` runs ad-hoc SQL through the real gated `federation.query`,
> renders `QueryResults.tsx`, and already has `SavedQueriesDialog`/`SaveQueryDialog`. A new `/t/$ws/query`
> surface would have double-built that page and split "where do I query?" across two rail entries. This
> slice therefore **upgrades the Datasources page**: the ad-hoc SQL textarea in `DatasourceDetail` is
> replaced by the full workbench (canvas builder from slice 1 + schema-aware editor from slice 2 + the
> existing results/saved-queries chrome), extracted as a reusable `QueryWorkbench` component so the same
> tree also mounts as a Data Studio pane. The original standalone-surface text below is kept for the parts
> that still apply (the component contract, the run path, the mandatory tests); the five-edit surface
> registration is **dropped** and replaced by §"Registration (retargeted)". If a standalone Query surface
> is ever wanted, the component is already the right shape — that becomes a one-file follow-up, not a rewrite.

Today the builder only lives *inside* the panel/dashboard editor. This slice gives it a second home —
the **Datasources page** — hosting the builder+editor over any datasource and rendering results in a grid.
Because the extracted `QueryWorkbench` takes the pane-friendly `{ ws, sel, onSel }` prop shape, the **same
component** mounts as a **Data Studio pane** via one `VIEW_PANES` line. This is the slice that runs real
queries, so it carries the **mandatory capability-deny and workspace-isolation gateway tests**.

### Registration (retargeted — replaces the five-edit standalone registration below)

1. **No `CoreSurface`/route/rail change.** The Datasources surface, route, and `CoreGate` cap stay as
   shipped.
2. `features/query-workbench/QueryWorkbench.tsx` — the extracted component (builder + editor + run bar +
   results), props `{ ws, sel, onSel }`. It does NOT own the datasource picker when embedded in
   `DatasourceDetail` (the page's selected datasource is passed in); it DOES render the picker when mounted
   as a Data Studio pane (accepts an optional `source` prop; absent ⇒ show picker).
3. `DatasourceDetail.tsx` — swap its raw-SQL textarea body for `<QueryWorkbench source={ds} …>`, keeping
   the existing `QueryResults`, `SavedQueriesDialog`, and probe/discovery chrome. Saved queries **shipped
   2026-07-06** on the platform's `query.*` verbs (`useDatasourceQueries` + `SaveQueryDialog` +
   `SavedQueriesDialog`, wired into the SQL editor header beside Run); the workbench reuses that wiring —
   `sel` is a saved-query id (`query.get`), Save goes through the same hook. `querydef.*` is dead.
4. Data Studio: one `VIEW_PANES` line keyed to the existing `datasources` surface def (or a dedicated
   `query` pane def if the dock needs a distinct icon/title — decide at build; either way no new route).
5. **Data page mount (decided 2026-07-06 — the "two SQL pages" question).** The Data page
   (`features/data/DataView.tsx` — the admin-only read-only DB browser: `store.tables`/`scan`/`graph`)
   was going to grow its own SQL box; instead it mounts the SAME component pinned to the surreal-local
   source: `<QueryWorkbench source="surreal-local">` (dialect `surreal`, `store.schema`/`store.query`).
   **Rejected: deleting the Data page and folding SurrealDB into Datasources** — the browser/graph has no
   home there, and it would blur the caps split (Data's scan verbs are admin-only; `store.query` /
   `federation.query` are member-level). **Rejected: a third Query page** — already rejected at peer
   review. Note the caps nuance: the Data page's Query area rides `mcp:store.query:call` (member-level),
   independent of the admin-only `store.scan` that gates the raw browser — whether the page shows Query
   to non-admins is a build-time nav/gate choice, not an architecture one.

## Goals

- **A first-class Query surface.** `coreRoute("/query", "query", …)` in the rail; deep-linkable
  `/t/$ws/query` and (when saved queries exist) `/t/$ws/query/$id`. Cap-gated by `CoreGate` like every core
  surface.
- **Author + run over any datasource.** A datasource picker (reuse `@nube/source-picker`), the
  builder+editor (slices 1–2) fed by the right schema/dialect for the picked source, a **Run** that calls
  `store.query` (surreal) or `federation.query` (federation) and renders rows.
- **Same component, three homes.** The routed page, the Data Studio pane, and (unchanged) the panel builder
  all reuse the builder tree. The view adds only the *surrounding chrome* (datasource picker + run +
  results), not a second builder.
- **Results grid.** Render `{columns, rows}` (or `viz.query` frames) with the shipped table render path —
  no new grid in v1.
- **Honest empty/deny states.** No datasource, no cap, or no rows each render a clear empty state; a denied
  Run shows the host error verbatim. Never fabricated rows.

## Non-goals

- **No NEW saved-query persistence.** Saved queries shipped (2026-07-06) on the `query.*` verbs via
  `useDatasourceQueries`/`SaveQueryDialog`/`SavedQueriesDialog`; the workbench reuses them. The view must
  still render correctly when `sel` is null / no queries are saved (fresh unsaved builder) — see the
  umbrella's §"Saved queries".
- **No new MCP verb / cap / table.** Reuses `store.schema`/`store.query`, `federation.schema`/`federation.query`
  over `/mcp/call`, and the surface's `CoreGate` cap lens.
- **No rich editable data grid.** Read-only results via the shipped table render path; the Tabularis
  tanstack grid is a harvest follow-up.
- **No query history / task manager / export** in v1 (harvest follow-ups).
- **No new dock engine.** Reuses the shipped Dockview `ViewDockPanel` adapter — one `VIEW_PANES` line, no
  pane-kind wiring.

## Intent / approach

### The view — `features/query-workbench/QueryWorkbenchView.tsx`

Props `{ ws: string; sel: string | null; onSel: (id: string | null) => void }` — the Flows/Rules
convention (verified: `FlowsView({ ws, flowId, onSelectFlow })`, `workbenchPanes.tsx` `ViewPaneProps`). Body
is an `AppPage`-led surface (the canonical shape from `ui-standards-scope.md`) with:

1. **A datasource picker** (`@nube/source-picker`'s `SourcePicker`/`SourceCombobox`) — pick surreal-local or
   a registered federation datasource. The picked source's `kind` yields the `dialect` (`surreal|standard`)
   and drives which schema loader feeds the editor (`readSchema()` vs. `useFederationSchema`) — the exact
   dispatch `QueryTab.tsx` already does; **reuse that composition, don't re-derive it**.
2. **The builder + editor** (slices 1–2): `<SqlQueryEditor dialect={…} schema={…} value={state} onChange=…>`.
3. **A Run bar** — Run button (+ the slice-2 splitter picks the statement under the cursor), a row-count/timing
   line, and a Format button (Code mode).
4. **A results area** — renders the run result. v1 reuses the shipped render path:
   - simplest: call `runFederationQuery`/`store.query` → `{columns, rows}` → the existing results table
     component (the one `TablePanel`/the data-console table browser already uses); OR
   - richer: build a transient `viz.query` panel and reuse `WidgetView` (as the panel builder's `PreviewPane`
     does) so charts are available too. **Recommendation: start with the `{columns, rows}` table**
     (simplest, one query, no panel plumbing); wire `viz.query`/charts as a fast follow if the team wants
     in-workbench charts. Confirm in OQ #1.

`index.ts` re-exports `QueryWorkbenchView` (barrel convention). The component is **workspace-argument-free
for data** — every call derives the workspace from the token; `ws` is a display/deep-link hint only.

### Standalone-view registration (five flat edits — TypeScript forces four of them)

Mirror the Flows registration exactly (verified map in `docs/prompts/data-studio/README.md` §1):

1. `CoreSurface` union — add `| "query"` in `ui/src/features/shell/NavRail.tsx`.
2. `SURFACE_DEF` — add the `query` entry (icon + label) in `ui/src/features/shell/surfaceDefs.ts` (the single
   source of truth for rail/dock/menu icon+title; TS forces the key).
3. `CORE_PATHS` — add `query: "/query"` in `ui/src/features/routing/surface.ts` (TS forces the key).
4. `coreRoute("/query", "query", () => <QueryWorkbench />)` in `ui/src/features/routing/createAppRouter.tsx`
   (wraps in `<CoreGate surface="query">`); plus a `queryDetailRoute` (`/query/$id`) + a `QueryWorkbenchSurface`
   wrapper that drives `sel`↔URL like `FlowsSurface` does — so deep links + back/forward work once saved
   queries exist. (Until then the detail route can 404-redirect to `/query`.)
5. Rail placement/order — `NavRail.tsx` owns grouping; add `query` where it reads best (near Data Studio /
   Data).

### Data Studio pane (one line)

Add `def("query", ({ ws, sel, onSel }) => <QueryWorkbenchView ws={ws} sel={sel} onSel={onSel} />)` to
`VIEW_PANES` in `ui/src/features/data-studio/workbenchPanes.tsx`. The "+ Open view" menu, dock tab, and icon
all derive from the `SURFACE_DEF` entry (step 2). `ViewDockPanel` mounts it in `EmbeddedPageContext` (header
folded) and persists `sel` via `api.updateParameters` — **no other Data Studio change**. `kind` must equal
the surface key (`"query"`) so the `SURFACE_DEF` lookup resolves (workbenchPanes convention).

### Run path — reuse, don't rebuild

- **Surreal**: `store.query { sql }` over `/mcp/call` (the shipped `sql.api.ts` runner), under
  `mcp:store.query:call`. Parse-allowlisted single SELECT, workspace-walled, row-capped at the host.
- **Federation**: `runFederationQuery(source, sql)` → `federation.query { source, sql }` over `/mcp/call`,
  under `mcp:federation.query:call`. SELECT-validated in the DataFusion sidecar, workspace-pinned, row-capped.
- The emitted SQL comes from `emitSql(dialect, builder)` (Builder) or the raw string (Code) — unchanged.

**Rejected: a `queryrun.*` convenience verb** that wraps both engines. It would widen a core chokepoint for
no gain — the view already dispatches on `kind` client-side (the same seam `QueryTab` uses), and the two
existing verbs are the right, already-capped doors. Adding a wrapper would also blur which cap gates a run.

## How it fits the core

- **Workspace is the hard wall (rule 6).** No data call takes a workspace arg; both engines pin the
  workspace at the host. The datasource picker lists only the caller's workspace's sources
  (`datasource.list` is workspace-scoped). **Mandatory workspace-isolation test:** a ws-B session's picker
  shows no ws-A source; a forged cross-tenant `federation.query` target is refused at the host and the UI
  surfaces the deny (no fabricated rows).
- **Capability-first (rule 5).** The surface is `CoreGate`-gated (`surface="query"`); a member without the
  surface cap can't route to it. Within it, schema/run degrade honestly on a missing `mcp:store.query:call`
  / `mcp:federation.query:call`. **Mandatory capability-deny test:** no run cap ⇒ empty schema/completion
  and a Run denied verbatim.
- **Symmetric nodes (rule 1).** Pure UI; identical edge/cloud.
- **One datastore (rule 2).** Two existing engines; no new one.
- **State vs motion (rule 3).** Request/response reads; no bus, no motion. (The view holds no durable state
  of its own until the user's saved-query record exists.)
- **Stateless / MCP contract / rule 10.** No extension change; emits existing MCP tools; dialect from `kind`,
  never a datasource name — the picker treats the id as opaque data.
- **No mocks (rule 9).** The gateway test drives the **real** spawned gateway + the **real** SQLite demo
  datasource (`make seed-demo-sqlite`), the `federation_sqlite_test.rs` pattern; the surreal path uses the
  real `mem://` store seeded with real rows. No `*.fake.ts`.
- **One responsibility per file (rule 8).** `QueryWorkbenchView.tsx` (the surface), `QueryRunBar.tsx`
  (run/format/count), `QueryResults.tsx` (results render), `useQueryRun.ts` (the run hook dispatching on
  kind). Registration edits are one line each in the flat maps.
- **API shape (§6.1).** N/A — no new verb; consumes `store.schema`/`federation.schema` (list/get reads) and
  `store.query`/`federation.query` (read). No batch/live-feed needed (a run is a synchronous bounded read).
- **Durability / secrets / SDK.** N/A.
- **Skill doc.** N/A for this slice (no new drivable verb). The surface is user-driven UI. (When the user's
  `querydef.*` verb lands, THAT needs `skills/querydef/SKILL.md` — recorded in the umbrella.)

## Example flow

1. Rail → **Query** → `/t/$ws/query`. `CoreGate` passes (member has the surface cap). `QueryWorkbenchView`
   opens with the default datasource, an empty builder, an empty results state.
2. The user picks `demo-buildings` (federation/sqlite). `dialect="standard"`; `useFederationSchema` loads its
   tables. The canvas (slice 1) + Code editor (slice 2) light up with that schema.
3. The user builds a join query (slice 1) or types SQL with completion (slice 2). The live preview shows the
   emitted standard SQL.
4. **Run** → `federation.query { source:"demo-buildings", sql }` under `mcp:federation.query:call`,
   workspace-pinned. Seeded rows render in the results table; a row-count + timing line shows.
5. The user opens Data Studio, clicks **+ Open view → Query**; the same view mounts as a dock pane, header
   folded, its `sel`/state persisted with the layout.
6. (User's saved-query work) they Save; the query gets an id; `/t/$ws/query/$id` deep-links it and it lists
   in "+ Open view".

## Testing plan

Per `scope/testing/testing-scope.md` — this is the slice that carries the **mandatory** categories (it runs
real queries). No mocks (§0).

### Gateway (real, rule 9 — the headline)

New `ui/src/features/query-workbench/QueryWorkbench.gateway.test.tsx` driving the real spawned gateway (+
the real SQLite demo datasource; follow the `DataStudioBuilderFlow.gateway.test.tsx` rect-stub discipline —
`@xyflow/react` and Dockview both measure layout in jsdom):

- **Headline — author → run → real rows.** Seed `demo-buildings`. Mount `QueryWorkbenchView`, pick the
  source, build a query (a join if slice 1 is in), Run; assert the emitted SQL is the standard string and the
  seeded rows render. Repeat once for a **surreal** local table via `store.query` (dialect swap, one component).
- **Capability-deny (mandatory §2.1).** A session without `mcp:federation.query:call`: the schema/completion
  is empty (discovery deny collapses to no rows) and a Run is denied at the host with the error surfaced
  verbatim — never fabricated rows. Same for `mcp:store.query:call` on the surreal path.
- **Workspace-isolation (mandatory §2.2).** A ws-B session: the datasource picker shows no ws-A source; a
  forged cross-tenant `federation.query` target is refused at the host; the UI surfaces the deny.
- **Standalone ≡ pane.** Mount the same component through `ViewDockPanel` (Data Studio pane) and assert the
  run path behaves identically (proving the one-component/three-homes claim).

### Unit

- **`useQueryRun`** — dispatches to `store.query` vs `runFederationQuery` by the picked source's `kind`; maps
  `{columns, rows}` to the results shape; surfaces an error object (not a throw) on deny.
- **Registration** — a `surfaceDefs`/`CORE_PATHS`/`VIEW_PANES` presence test (the pattern
  `workbenchPanes.test.ts` uses) so the new `query` key is wired in all maps.

### Backend regression

None — no backend change. The existing `federation_sqlite_test.rs` (federation deny + ws-isolation) and the
`store.query` host tests stay green as the contract this rides.

## Risks & hard problems

- **`@xyflow/react` + Dockview in jsdom** — both measure layout; the gateway test must rect-stub (copy the
  `DataStudioBuilderFlow.gateway.test.tsx` pattern). This is the most likely test-flake source.
- **Results render choice** — the `{columns, rows}` table is simplest; wiring `viz.query`/charts is more
  plumbing. v1 recommendation: table first (OQ #1).
- **`sel` wiring** — RESOLVED: saved queries exist (`query.*`, shipped 2026-07-06). `sel` is a saved-query
  id resolved via `query.get`; a null `sel` renders a fresh builder. The remaining wiring is only the
  `?query=<id>` deep-link on the datasources route.
- **Rail crowding** — one more core surface. Placement is `NavRail`'s call; group with Data/Data Studio.
- **Two run caps, one surface** — the deny UX must distinguish "no surface cap" (can't route) from "no run
  cap" (routed, run denied). The `CoreGate` handles the first; the results-area error handles the second.

## Open questions — RESOLVED (peer review 2026-07-06)

1. **Results render** — DECIDED: `{columns, rows}` table first, reusing the page's existing
   `QueryResults.tsx`; `viz.query`/charts are a fast follow.
2. **Surface name/route** — MOOT: retargeted onto the existing Datasources surface; no new route or
   `CoreSurface` key.
3. **Detail route** — MOOT for a new route. Deep-linking a saved query becomes a `?query=<id>` search param
   on the existing datasources route when the saved-query story consolidates.
4. **Datasource picker default (pane mount only)** — DECIDED: surreal-local default; remember-last lands
   later as a nullable axis on `lb_prefs::Prefs` (whole-struct fold, NOT a KV key — established prefs
   convention).

## Related

- [`query-builder-10x-scope.md`](query-builder-10x-scope.md) — the umbrella; §"Saved queries" (the seam this view leaves for the user).
- [`visual-canvas-builder-scope.md`](visual-canvas-builder-scope.md) · [`sql-editor-10x-scope.md`](sql-editor-10x-scope.md) — the builder + editor this view hosts.
- `docs/prompts/data-studio/README.md` — the standalone-view + pages-as-panes registration map (the five edits, the `ViewDockPanel` adapter).
- `docs/scope/frontend/data-studio-10x-scope.md` — the Data Studio surface this pane joins.
- `docs/scope/frontend/routing-scope.md` · `docs/scope/frontend/nav-rail-scope.md` — the routing + rail conventions the registration follows.
- `docs/scope/frontend/system-catalog-scope.md` + `packages/source-picker/` — the datasource picker this view reuses.
- `ui/src/features/flows/FlowsView.tsx` · `ui/src/features/routing/createAppRouter.tsx` (`coreRoute`, `FlowsSurface`) — the standalone-view reference to mirror.
- `ui/src/features/data-studio/{workbenchPanes.tsx,panes/ViewDockPanel.tsx}` — the pane registration + adapter.
- `ui/src/features/panel-builder/tabs/QueryTab.tsx` · `useFederationSchema.ts` · `useLocalSchema.ts` — the per-`kind` datasource→dialect→schema dispatch to reuse.
- `rust/crates/host/tests/federation_sqlite_test.rs` + `make seed-demo-sqlite` — the rule-9 real-datasource fixture.
</content>
