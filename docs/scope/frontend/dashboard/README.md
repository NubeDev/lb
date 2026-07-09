# Dashboard scope index

Status: scope index. Durable shipped behavior lives in
[`public/frontend/dashboard.md`](../../../public/frontend/dashboard.md).

This directory groups the dashboard-specific frontend scopes. The older flat scope files remain linked
because existing session docs point at them; new dashboard notes should live here.

## Read order

1. [`../dashboard-scope.md`](../dashboard-scope.md) - the original build-ready Phase 1 scope: dashboard
   records, `dashboard.*` verbs, grid layout, built-in widgets, and live series streams.
2. [`widgets-scope.md`](widgets-scope.md) - the widget-focused scope: built-in widgets that render today,
   extension widget tiles that surface through `ext.list`, and the remaining grid-cell renderer work.
3. [`../dashboard-widgets-scope.md`](../dashboard-widgets-scope.md) - the deeper federation contract:
   `[[widget]]`, bridge rules, trust tiers, and the no-token/no-db invariant.
3b. [`widget-builder-scope.md`](widget-builder-scope.md) - the **v2 generalization**: a widget binds any
   *view* (chart/table/stat/gauge/Observable Plot/D3/JSX template/control) to any *MCP tool* the install
   grant allows (read **or** write), authored in a rubix-cube-style builder, plus extension-shipped
   `[[widget]]` tiles. Supersedes the read-only/four-verb stance of (3).
3c. [`widget-palette-scope.md`](widget-palette-scope.md) - the **last-mile discovery slice**: surface a
   packaged `[[widget]]` tile (e.g. `proof-panel`'s Proof Ping) in the builder's source picker, gated to
   editors with `mcp:dashboard.save:call`. The renderer + bridge ship in (3b); this adds the palette entry
   + the permission gate so a user actually *gets a new option when adding a widget*.
3d. [`widget-config-vars-scope.md`](widget-config-vars-scope.md) — **SHIPPED (2026-06-28)**. **widget
   settings/config + a Grafana-style variable system**: per-cell edit (name/options/reconfigure), dashboard **variables**
   (`$var`/`${var}`/`[[var]]` + built-ins `$__from`/`$__interval`/`${__user.login}`/…, URL-synced
   `?var-host=`), **auto-refresh** + **live events**, a **JSON payload builder** (send to an extension /
   over the bus), all on **one shared interpolation library extensions reuse** and **one variable model**
   resolving from SurrealDB / SSE / Zenoh / extensions / JSON. Names the one new backend API: generic
   `bus.publish` / `bus.watch`.
3e. [`viz/`](viz/README.md) — the **Grafana-compatible visualization** slice (scope, the ask): adopt
   Grafana's panel/`fieldConfig`/transformation/datasource model and dashboard JSON so charts gain the full
   standard option surface, render units/numbers/dates through **user-prefs**, query **any** datasource (not
   just native SurrealDB), and **import/export Grafana dashboard JSON**. One file per part —
   [`README.md`](viz/README.md) (umbrella + the cell↔panel reconciliation),
   [`panel-model-scope.md`](viz/panel-model-scope.md) (the additive v3 shape),
   [`chart-types-scope.md`](viz/chart-types-scope.md) (the standard chart set),
   [`field-config-scope.md`](viz/field-config-scope.md) (chart options + the user-prefs bridge),
   [`transformations-scope.md`](viz/transformations-scope.md) (the pipeline),
   [`datasource-binding-scope.md`](viz/datasource-binding-scope.md) (datasources beyond native SurrealDB),
   [`import-export-scope.md`](viz/import-export-scope.md) (Grafana JSON in/out), and
   [`panel-editor-scope.md`](viz/panel-editor-scope.md) (the editor UX + the add≡edit parity fix). Additive
   over the shipped v2 contract. **`viz/panel-wizard-scope.md`** adds the create-flow **wizard**
   (preview-per-option, one engine with the Field tab) — the Field-tab baseline audit
   ([`debugging/frontend/field-tab-options-that-do-nothing.md`](../../../debugging/frontend/field-tab-options-that-do-nothing.md))
   is its input.
3f. [`source-picker-package-scope.md`](source-picker-package-scope.md) — **extract the shipped source
   picker into a reusable `@nube/source-picker` package** so a user (or AI) can select a value from the
   DB / datasources / Zenoh (live series) / flows the SAME way the dashboard does, from OUTSIDE the
   dashboard (first new consumer: the `thecrew` graphics-canvas extension). Headless-first + a
   dependency-injected `SourceLoaders` seam so one picker works from both the shell (gateway/Tauri) and an
   extension (its bridge); dashboard migrates first (parity), thecrew second. Zero core additions.
3g. [`../../genui/genui-scope.md`](../../genui/genui-scope.md) — the **AI-authored widget**: a
   `view:"genui"` cell whose layout the workspace agent designs from a prompt (streamed live preview
   over the RunEvent SSE; the emission is parsed/normalized **once at accept** and the versioned,
   typed IR is what `dashboard.save` persists), rendered from a reusable `@nube/genui` package
   (A2UI-shaped IR + our own catalog renderer; OpenUI-Lang authoring adapter in v1) in the sandboxed
   iframe tier with a concrete in-process promotion checklist, with steady-state data through
   ordinary v3 `sources[]` — the agent authors, it never serves. Lives as its own top-level topic
   because channels rich responses and other surfaces reuse the package (the `source-picker`
   extraction precedent).
3h. [`ext-widget-source-binding-scope.md`](ext-widget-source-binding-scope.md) — **extension widgets
   over any source (frames-in)**: make an `ext:<id>/<widget>` tile a first-class *view* over the v3
   panel model — the cell carries ordinary `sources[]` (SurrealDB `store.query` / series history+live /
   federation datasources / flow node ports), the shell resolves them via `viz.query` under the
   *viewer's* grant, and the tile receives resolved frames (`ctx v3` + optional `update()`), never
   fetching platform data itself. Additive over the v2 mount contract; zero new extension caps.
3i. [`library-panels-scope.md`](library-panels-scope.md) — **panels as their own asset**: a
   `panel:{id}` record (the non-layout half of a `Cell` — the v3 spec) with `panel.*` verbs + S4
   sharing, referenced from dashboards via an additive `panel_ref` cell field (edit once, every
   referencing dashboard updates; explicit Unlink to fork) and rendered **standalone** on a
   `/t/$ws/panel/{id}` page (a chart with no dashboard — the page a nav entry or shared link points
   at). Sharing a panel never widens data access — `sources[]` re-check under the viewer's caps.
3j. [`reusable-pages-scope.md`](reusable-pages-scope.md) — **SHIPPED (2026-07-03)** — **one page, reused many times**: a
   template dashboard is an ordinary dashboard whose `variables[]` are its parameters
   (`Variable.required` additive flag → an honest "select a site" gate); an **instance is a binding,
   never a copy** — carried by the URL (`?var-`, shipped), a nav `dashboard` entry's pinned `vars`
   (additive), or a **`template-group`** nav entry that fans out one link per tag-facet/option value
   at `nav.resolve` (tag a new site → a new page, zero edits). No new tables, verbs, or caps.
   Build order: nav builder → library panels → this.
3k. [`data-studio-ux-scope.md`](data-studio-ux-scope.md) — **the Data Studio editing loop**
   (scope, the ask): make query→see-data Grafana-Explore-grade — query status bar (rows/
   duration/error/why-empty), data inspector, real Run semantics + auto-run toggle
   ("Apply" renamed to "Save to tab"), a searchable source combobox, and **edit-without-
   requery**: split `viz.query` fetch vs shape (additive inline-`frames` compute-only mode,
   same cap) + a freeze-current-data toggle, so option/transform edits re-shape cached
   frames instead of re-hitting the datasource.
3l. [`render-template-inprocess-scope.md`](render-template-inprocess-scope.md) — **SHIPPED (2026-07-05)**
    — **the render-template widget, in-process (no iframe)**: promoted the eval-free `template` engine
    off the sandboxed-iframe tier to a first-class **in-process** view (`TemplateView`, sibling of
    `GenUiView`) — same `usePanelData` rows (so it binds ANY source the panel-data hook resolves, with
    no per-source code), same leashed host-re-checked write bridge, editable in Data Studio via the
    (formerly orphaned) CodeMirror HTML editor. The one new guard replacing the sandbox: **DOMPurify**
    (`sanitizeTemplateHtml.ts`, one file) for the author HTML (+ the existing `dashboard.save`/
    `template.save` cap as the authoring trust gate); an exhaustive **XSS-vector suite** is the
    definition of done. `plot`/`d3` stay on the iframe tier (they `eval`). One host gap surfaced (a view
    bound to `rules.run` renders empty for every view — out of scope, tracked in
    `debugging/frontend/rules-as-source-render-path-empty.md`). See the shipped iframe reference
    [`render-template-widget.md`](render-template-widget.md).
3m. [`rules-as-source-scope.md`](rules-as-source-scope.md) — **a saved rule is a picker source**:
    the Rules group in the source picker (`rules.run {rule_id}` + typed params) — the **picker**
    half shipped 2026-07-05; the **render** half (a view bound to `rules.run` actually drawing the
    rule's rows) is blocked by a host gap and is scoped as its own slice:
    [`rules-for-widgets-scope.md`](rules-for-widgets-scope.md) — fix the `viz.query` recursive
    dispatch + unwrap the `RuleOutput` envelope, add read-only panel runs (`route:false` so a
    30 s auto-refresh never spams the Inbox), and ship chart-return helpers in the cage
    (`timeseries`/`wide`/`category`) so "make this rule chartable" is one line.
3n. [`panel-wizard-source-discoverability-scope.md`](panel-wizard-source-discoverability-scope.md) —
    **SHIPPED (2026-07-09)** — **"bind this panel to a saved RULE" is now a one-glance path in the
    new-panel wizard**. The picker + render halves shipped (3m), but the wizard's step-1
    Source chooser only names "rule" in the *Workspace source* card's subtitle and buries the Rules
    group seventh in a flattened combobox — a user hunting a rule clicks *Datasource* and lands in the
    SQL workbench. Recommendation: keep the three-bucket cards (no 5th "Rule" card — rejected: it
    fractures one generic picker seam into per-kind cards), rewrite the card subtitle to front-load
    "rule/series/saved query", reorder the Rules group to lead the workspace sub-picker, and add an
    empty-Rules line. Labelling-only, CLAUDE §10 held.
4. [`../../extensions/ui-federation-scope.md`](../../extensions/ui-federation-scope.md) - the broader
   extension UI page/federation model that widgets narrow down to one dashboard cell.

## What is shipped

- The **Data Studio editing loop** ([`data-studio-ux-scope.md`](data-studio-ux-scope.md),
  SHIPPED 2026-07-04): a query **status bar** (rows/frames/duration/error inline/why-empty),
  real **Run/Refresh** semantics for every datasource + ⌘-Enter (Data Studio's save is now
  "Save to tab", not "Apply"), a **searchable source combobox** (`@nube/source-picker`), and
  **edit-without-requery** — `viz.query` split into fetch (sources → raw frames) vs shape (an
  additive inline-`frames` compute-only mode, same `mcp:viz.query:call` cap) so a
  field/override/transform edit reshapes cached frames instead of re-hitting the datasource,
  plus a **freeze** ("use current data") toggle. See
  [`public/frontend/dashboard.md`](../../../public/frontend/dashboard.md) → "Data Studio
  editing loop".
- The first-party dashboard surface exists in the shell: roster, create/select/delete, visibility, grid
  layout, drag/resize persistence, and built-in chart/stat/gauge widgets.
- Built-in widgets bind to real series either by explicit series name or by tag query. They backfill via
  store reads and receive live samples through the series SSE stream.
- Extension manifests may declare several `[[widget]]` tiles. Those tiles persist on the `Install`,
  are narrowed to the approved grant, and surface in `ext.list`.
- **Rule discoverability in the new-panel wizard** ([`panel-wizard-source-discoverability-scope.md`](panel-wizard-source-discoverability-scope.md),
  SHIPPED 2026-07-09): the wizard's step-1 **Workspace source** card front-loads "rule / series / saved
  query", its source list opens with the **Rules** group first (not buried seventh), and an empty
  workspace shows "No saved rules yet — create one in Rules". Picking a rule emits the shipped
  `{tool:"rules.run", args:{rule_id, route:false}}`. Labelling/order only — one generic picker seam, no
  per-source branch (CLAUDE §10). See
  [`public/dashboard/dashboard.md`](../../../../doc-site/content/public/dashboard/dashboard.md) →
  "New-panel wizard — binding a panel to a saved rule".

## What is not shipped yet

- Federated extension widgets **render** inside dashboard grid cells (`WidgetView` → `ExtWidget` mounts an
  `ext:<id>/<widget>` cell through the v2 bridge, in-process by trusted publisher key or in a sandboxed
  iframe otherwise — widget-builder v2) **and are now addable from the builder palette** — the discovery
  gap is **closed**: [`widget-palette-scope.md`](widget-palette-scope.md) **shipped** the "Extension
  widgets" picker group (one entry per `[[widget]]` tile) + the editor-only (`mcp:dashboard.save:call`)
  add gate. See [`public/frontend/dashboard.md`](../../../public/frontend/dashboard.md) → "Extension
  widgets in the palette".
- The **Grafana-compatible visualization** layer ([`viz/`](viz/README.md)) is **scoped, not built**: the
  standard chart set with the full option surface, the `fieldConfig`/transformation/datasource model,
  user-prefs-driven formatting, and Grafana dashboard JSON import/export. Phase 1 is `timeseries` end to end
  (the panel-model spine + field-config + the redesigned editor). Additive over the shipped v2 cell.
- The external-data **reference extensions** whose tools/tiles a widget would call (timescale, mqtt-bridge)
  are blocked on separate platform fixes (native host-callback, `net:*`, `kv.*`, secrets) —
  [`../../extensions/reference-extensions-scope.md`](../../extensions/reference-extensions-scope.md). The
  dashboard is ready for them; they are not yet built.
- The **flow⇄dashboard binding UX** — a flow-aware source picker (pick flow → node → port/slot), switch/
  slider wired automatically, and **structured JSON in *and* out** — is **scoped, not built**:
  [`../../flows/flow-dashboard-binding-ux-scope.md`](../../flows/flow-dashboard-binding-ux-scope.md). It
  extends this dashboard's source picker + control views; the underlying `flows.inject`/`flows.node_state`
  mechanism already shipped ([`../../flows/dashboard-binding-scope.md`](../../flows/dashboard-binding-scope.md)).
- **Extension widgets bound to platform sources** — an `ext:` tile receiving resolved frames from
  cell `sources[]` (datasource / flow / series / SurrealDB) instead of only its own manifest tools —
  is **scoped, not built**: [`ext-widget-source-binding-scope.md`](ext-widget-source-binding-scope.md).
- The **widget catalog for AI authors** — a `dashboard.catalog` MCP verb exposing the palette (built-in
  views with a per-widget version + config schema, generically-folded ext tiles, genui components) **plus**
  host-side save-validation that rejects a cell with an unknown `view` — so the AI stops "adding widgets
  that don't exist" — is **scoped, not built**: [`widget-catalog-scope.md`](widget-catalog-scope.md). The
  catalog is a **host-owned JSON data file** (`widget_catalog.json`, the `genui_catalog.json` pattern) —
  backend-driven and client-agnostic (the web UI **and** the RN app render from it). Sibling of the
  (shipped) human `widget-palette-scope.md`.
- **Rules rendering in widgets** — a panel/widget bound to a saved rule renders zero rows for every
  view (the `viz.query` → `rules.run` recursive dispatch fails and the `RuleOutput` envelope is not
  unwrapped) — **scoped, not built**: [`rules-for-widgets-scope.md`](rules-for-widgets-scope.md)
  (fix the two host layers, `route:false` read-only panel runs, chart-return helpers in the cage).
  The picker half (Rules group + typed params) already shipped
  ([`rules-as-source-scope.md`](rules-as-source-scope.md)).
- The **reusable source-picker package** (`@nube/source-picker`) — extract the shipped picker (db /
  datasources / Zenoh / flows / extension widgets) so surfaces OUTSIDE the dashboard reuse it — is
  **scoped, not built**: [`source-picker-package-scope.md`](source-picker-package-scope.md). Dashboard
  refactors onto it first (parity), then `thecrew` consumes it.
## Authoring rule

Keep new docs in this directory focused on dashboard scope. When a slice ships, promote the stable facts
to [`public/frontend/dashboard.md`](../../../public/frontend/dashboard.md) and leave session-specific
debugging and command output in `docs/sessions/`.
