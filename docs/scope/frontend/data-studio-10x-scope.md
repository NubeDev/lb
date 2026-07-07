# Frontend scope — Data Studio 10x: Dockview workbench, pages-as-panes, query-first builder

Status: **SHIPPED** (2026-07-05) — promoted to [`public/frontend/data-studio.md`](../../public/frontend/data-studio.md);
session log: [`sessions/frontend/data-studio-10x-session.md`](../../sessions/frontend/data-studio-10x-session.md).
Follow-on to [`data-studio-scope.md`](data-studio-scope.md) (v2/v3 shipped) and the
[data-studio-rail session](../../sessions/frontend/data-studio-rail-session.md) (2026-07-05).

Data Studio works but doesn't yet earn its "workbench" name. Three pains, straight from use:
**(1)** the layout engine (flexlayout-react) fights us — an unmaintained theme bridge, rotated
border strips, jsdom hacks, barely-visible tabs; **(2)** testing data means bouncing between the
Data, Datasources, Rules, and Flows *pages* because the studio can't show them — the debugging loop
lives across four routes; **(3)** the builder tab shows its whole Grafana-depth option surface at
once (seven chrome bands before any rows) — dense where it should be a flow. The ask: replace the
dock engine with **Dockview**, let the studio open the app's **own pages as panes** (Flows top,
Rules bottom, builder beside — one saved arrangement), and rework the builder into a
**query-first → visual-viz-gallery → options-on-demand** flow with an honest seeded-demo-data
preview.

## Goals

1. **Dockview as the one dock engine** (`dockview` npm, MIT, React-first): tabs, nested
   top/bottom/left/right splits, floating groups, maximize, popout, JSON serialize/restore —
   replacing flexlayout-react everywhere it's mounted (Data Studio only today). Theme via
   Dockview's CSS custom properties aliased to the shell tokens (the same packages/* CSS rules the
   current bridge follows). Tab titles capped + ellipsized with full-title tooltips (parity with
   the fix shipped 2026-07-05).
2. **Pages-as-panes.** A "+ Open view" header menu lists the core surfaces — Flows, Rules, Data,
   Datasources, Ingest — plus "New panel". Each opens as a dock pane mounting the **real routed
   view component** (`FlowsView`, `RulesView`, `DataView`, …): same code path, same gateway, same
   caps — never a re-implementation. Panes split/stack/float freely; the whole arrangement (which
   views + geometry + every builder draft) persists per member via the shipped `layout.get/set`.
   An `embedded` mode on `AppPage` suppresses the view's own full-width header inside a pane (the
   dock tab is the title bar); the standalone routes keep it.
3. **Query-first builder flow** (progressive disclosure, all inside `panel-builder`):
   - *Stage 1 — source picked:* one compact toolbar (inline title, Run, one Save split-button:
     to-tab / to-library) + the query editor, focused, prefilled. No preview, no viz pills, no
     options rail until rows exist. The "Saved as library panel …" banner becomes a badge.
   - *Stage 2 — rows returned:* a **viz gallery** replaces the text pill row — one thumbnail card
     per widget type, each a live mini-render of the caller's ACTUAL frames through the one
     `viz.query`/`WidgetHost` path (no second renderer). Click a card → full preview.
   - *Stage 3 — refine on demand:* Query/Plot/Transform/Panel options/Field/Overrides fold into
     one collapsed, searchable Options drawer. Power depth intact, default cost zero.
4. **Demo data, honestly seeded** (rule 9 — no client-fabricated frames): when a query returns
   zero rows, the empty preview offers **"Preview with demo data"** — real records, real engine.
   The canonical demo source is the **SQLite building dataset** — shipped 2026-07-05 as
   [`../datasources/sqlite-datasource-demo-scope.md`](../datasources/sqlite-datasource-demo-scope.md)
   (`make seed-demo-sqlite`, no Docker) — with the Timescale `seed.py` path as the full-size
   variant. Demo state is clearly badged and toggles off the moment the user's own query has rows.
   Same render path either way.
5. **The rail's Sources tab becomes a `CatalogExplorer` host** (the workspace system catalog —
   [`system-catalog-scope.md`](system-catalog-scope.md)): browse datasources → tables → columns,
   series, flows, rules, channels, insights as ONE tree with the catalog's honest per-section
   deny/loading/empty states, click → open a builder tab on the picked entry (the studio's
   `onSelect` mapping, exactly like the rules panel's Rhai mapping). Replaces the bare
   `SourcePicker` select in `SourcesPane` — the studio composes the shipped skin, it does not grow
   its own tree.

## Non-goals

- **No new query/render substrate.** Everything still routes `viz.query`/`usePanelData`/
  `WidgetHost`/`@nube/source-picker`. The gallery renders through the shipped path with small
  frames — not a thumbnail renderer.
- **No app-shell docking.** The dock lives inside Data Studio; the shell nav/routes are untouched.
  (A "whole app is a dock" VS Code shell was considered and rejected: it rewrites the routing and
  cap-gating story for every surface to serve one power page.)
- **No new host verbs/tables for phases 1–3 core.** Layout rides `layout.*`; pages ride their own
  shipped verbs. The ONLY candidate addition is an admin-gated demo-seed verb (OQ2).
- **No extension special-casing** (rule 10): extension pages join the "+ Open view" menu later via
  the generic `ext.list` discovery — ids stay opaque; not in the first cut.
- **Not a Grafana option-surface redesign.** The drawer reorganizes *when* options appear, not
  what they are; the fieldConfig/overrides model is unchanged.

## Intent / approach

The key idea: **the studio is a composition surface, so compose — don't rebuild.** Dockview is
infrastructure-only (phase 1); the 10x is phase 2's realization that our pages are already
self-contained cap-gated components taking `ws`, i.e. dock panels that already exist. Phase 3 is
pure `panel-builder` layout re-sequencing over the shipped machine (panel-kit state, one
(de)serializer, one query hook).

Alternative rejected: keeping flexlayout-react and only restyling. The engine is the ceiling —
no floating groups, a vendored theme bridge we already patch around, and an API that made the
border-dock mistake natural. Dockview's serialize shape is versioned JSON like today's, so the
`useWorkbenchLayout` seam survives; only `workbenchModel.ts` changes vocabulary.

Phases ship independently, in order: **(1) engine swap → (2) pages-as-panes → (3) builder flow +
demo data.** Phase 3 can land before 2 if priorities flip; 2 depends on 1.

## How it fits the core

- **Tenancy / isolation:** unchanged walls. Layout records stay `ui_layout:[ws, user, surface]`
  (member-owned). Every embedded page re-checks its own caps under the caller exactly as the
  routed page does — embedding changes WHERE a view mounts, not its authority. Demo seeds write
  only into the caller's workspace.
- **Capabilities:** no new caps for phases 1–2 (a pane the caller lacks caps for renders that
  page's own denied/empty state — same as navigating there). "+ Open view" lists only surfaces
  the caller's route gating (`allowed`) already grants. Demo-seed verb (if built, OQ2) gets its
  own cap + deny test.
- **Placement:** frontend-only; symmetric (browser + Tauri identical). N/A to node roles.
- **MCP surface:** consumed only — `layout.get/set`, each pane's own verbs, `viz.query`,
  `panel.*`. Read verbs + the existing SSE feeds; no new CRUD, no batch. The optional demo-seed
  verb is a single bounded write (small fixed record set — synchronous, no job needed).
- **Data (SurrealDB):** the layout record's `model` JSON changes shape (Dockview's). Version it:
  persist `{engine:"dockview", model}` — a legacy flexlayout blob (no `engine` tag) falls back to
  the default workbench (the fallback already shipped; drafts inside old layouts are the only
  loss, accepted — the library holds anything saved).
- **Bus (Zenoh):** N/A — panes reuse their pages' existing feeds.
- **No mocks / fake backends (rule 9):** the demo-data toggle is REAL seeded records (workspace
  `iot_demo` seed; the Timescale dataset behind a real federation datasource). No synthetic frames
  in the client, ever; the badge keeps it honest to the user too.
- **One responsibility per file:** pane registry (`workbenchPanes.ts` — kind→component map, ids
  as data), the open-view menu, the `AppPage` embedded prop, `VizGallery.tsx`, the options
  drawer, and the demo-data hook are each their own file; `BuilderPane` sheds what they take.
- **SDK/WIT impact:** none.
- **Skill doc:** N/A — no new agent-drivable surface (verbs consumed are already documented);
  revisit only if the demo-seed verb ships (then it joins the existing testing/seed docs).

## Example flow

1. Ada opens Data Studio. Her saved layout restores: Flows pane (top-left), Rules pane
   (bottom-left), a builder tab on `cooler.temp` (right).
2. She watches the flow fire in the Flows pane (its own live node-state feed), sees the rule
   trigger below, and her builder preview refresh on Run — no route changes.
3. "+ Open view → Datasources", drags it as a bottom tab next to Rules, checks the Timescale
   source is healthy.
4. New panel from the rail's Sources tab → the builder opens **query-first**: SQL editor focused,
   prefilled. Her query returns 0 rows → the empty preview offers "Preview with demo data"; she
   toggles it, the viz gallery renders demo-seeded frames (badged *demo*), she picks Gauge from
   the thumbnails, fixes her WHERE clause, demo auto-yields to her 500 real rows.
5. Save → library panel; the arrangement (all four panes + the draft) persists to her member-owned
   layout record. Ben, same workspace, still sees HIS layout.

## Testing plan

Real gateway/store throughout (`testing-scope.md`; the existing `DataStudio.gateway.test.tsx`
pattern — rect-stubbing carries over since Dockview also measures DOM):

- **Mandatory:** capability-deny (a caps-stripped session: pane menu omits ungranted surfaces; an
  embedded pane's verbs still deny server-side; demo-seed verb denied without its cap) and
  workspace-isolation (layout + seeded demo records never cross to ws-B — extend the shipped
  isolation case).
- Phase 1: existing 7 gateway cases stay green on Dockview (open-from-source, library round-trip,
  member-owned layout, isolation, deny, SQL-editor conditional, rail collapse); legacy-layout
  fallback (a stored flexlayout blob → default workbench, no crash).
- Phase 2: open Flows+Rules panes → both render their REAL views against the gateway; layout
  round-trip restores both; `AppPage` embedded mode (no header in-pane, header intact on the
  route).
- Phase 3: query-first (no options rail pre-rows), gallery renders per-type from one real query,
  demo toggle (0 rows → demo frames badged; real rows → demo off), save round-trips unchanged
  (`panel.save`/`panel.get`).
- Units: pane registry, layout-record versioning, gallery type-mapping, drawer search.

## Risks & hard problems

- **Layout migration:** silent draft loss from v2 blobs will surprise someone — surface a one-time
  "layout was reset (engine upgrade)" notice rather than resetting silently.
- **Pages weren't written to be multi-mounted:** two Flows panes = two canvases + duplicate
  polling/SSE. First cut: allow one pane per view kind (menu disables an open one). Shared
  subscriptions are the later fix, not a blocker.
- **Recursive embedding:** Data Studio must not list itself; the registry excludes the host
  surface (and anything FlexLayout-era CSS assumed — delete the old bridge with the engine).
- **Gallery cost:** N thumbnail renders per query. Render from the ALREADY-FETCHED frames (one
  query, N cheap views) — never N queries; assert that in tests.
- **Demo-data trust:** an unbadged demo frame is a lie in a control surface. The badge + auto-yield
  are correctness requirements, not polish.

## Open questions

1. **Pane granularity for phase 2:** ~~whole pages only, or also sub-panes (a single flow's canvas,
   one rule's panel)?~~ **Resolved the recommended way and SHIPPED (2026-07-05)** — whole pages first
   (the registry lists Flows/Rules/Data/Datasources/Ingest as dock panes, each mounting the REAL routed
   view). Sub-panes only if the whole-page pane proves too coarse in use. ONE pane per view kind in
   the first cut (the menu re-activates an open pane) — pages weren't written to be multi-mounted, so
   shared subscriptions are the later fix.
2. **Demo seed verb:** ~~reuse the test-gateway's `iot_demo` seed as a real admin-gated host verb,
   or docs-only?~~ **Resolved the lite way, and SHIPPED (2026-07-05)** — the demo datasource is a
   **SQLite file** seeded with the same building dataset (`make seed-demo-sqlite`, no Docker, no new
   host verb); see
   [`../datasources/sqlite-datasource-demo-scope.md`](../datasources/sqlite-datasource-demo-scope.md).
   Remaining sliver: whether workspace-*series* demo (the `iot_demo` records) also needs a
   one-click seed, or the SQLite datasource alone carries the demo toggle. Recommend the latter
   first — one demo source is enough to prove the gallery.
3. **Viz gallery coverage:** ~~all ~10 widget types as thumbnails, or the 6 chart-likes with
   Table/AI-widget/Template as labeled cards (no mini-render)?~~ **Resolved the recommended way and
   SHIPPED (2026-07-05)** — the split: 6 chart-likes (`timeseries`/`barchart`/`stat`/`gauge`/
   `bargauge`/`piechart`) get a live mini-render through the one `WidgetView`/`viz.query` path;
   Table / AI-widget / Template are labeled cards (a Template thumbnail is noise). Pinned by
   `VizGallery.test.tsx` (9 cards; shape-gating mirrors `VizPicker`).
4. **Popout windows** (Dockview supports them): ~~keep tab-popout parity with today, or drop popout
   until someone asks?~~ **Resolved: keep — Dockview makes it cheaper, and the rename affordance
   stays prompt-based for now** (double-click a tab → `window.prompt`). Shipped 2026-07-05. A future
   polish slice could swap the prompt for an inline editable tab title (recorded, not a gap).

## Related

- Parent: [`data-studio-scope.md`](data-studio-scope.md) (v2/v3, shipped) ·
  [`dashboard/data-studio-ux-scope.md`](dashboard/data-studio-ux-scope.md) (fetch/shape decoupling)
- Shipped substrate: [`widget-kit-scope.md`](widget-kit-scope.md), panel-kit
  (`ui/src/lib/panel-kit`), the roster-rail kit
  ([data-studio-rail session](../../sessions/frontend/data-studio-rail-session.md))
- Catalog: [`system-catalog-scope.md`](system-catalog-scope.md) — the `CatalogExplorer` tree the
  rail's Sources tab hosts (goal 5); the studio consumes the package skin, never a private tree
- Demo source (shipped): [`../datasources/sqlite-datasource-demo-scope.md`](../datasources/sqlite-datasource-demo-scope.md)
- Public: [`../../public/frontend/data-studio.md`](../../public/frontend/data-studio.md)
- Demo data: `docker/postgres/seed.py` (Timescale building dataset), `iot_demo` seed
  (`ui/src/test/gateway-session.ts`)
- Engine: <https://dockview.dev/> (`dockview` — React, MIT)
