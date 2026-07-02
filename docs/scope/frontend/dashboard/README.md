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
   over the shipped v2 contract.
3f. [`source-picker-package-scope.md`](source-picker-package-scope.md) — **extract the shipped source
   picker into a reusable `@nube/source-picker` package** so a user (or AI) can select a value from the
   DB / datasources / Zenoh (live series) / flows the SAME way the dashboard does, from OUTSIDE the
   dashboard (first new consumer: the `thecrew` graphics-canvas extension). Headless-first + a
   dependency-injected `SourceLoaders` seam so one picker works from both the shell (gateway/Tauri) and an
   extension (its bridge); dashboard migrates first (parity), thecrew second. Zero core additions.
3g. [`../../genui/genui-scope.md`](../../genui/genui-scope.md) — the **AI-authored widget**: a
   `view:"genui"` cell whose layout the workspace agent designs from a prompt (streamed live preview
   over the RunEvent SSE, persisted via the normal `dashboard.save`), rendered from a reusable
   `@nube/genui` package (A2UI-shaped IR + OpenUI-Lang/A2UI adapters + our own catalog renderer) in
   the sandboxed iframe tier, with steady-state data through ordinary v3 `sources[]` — the agent
   authors, it never serves. Lives as its own top-level topic because channels rich responses and
   other surfaces reuse the package (the `source-picker` extraction precedent).
4. [`../../extensions/ui-federation-scope.md`](../../extensions/ui-federation-scope.md) - the broader
   extension UI page/federation model that widgets narrow down to one dashboard cell.

## What is shipped

- The first-party dashboard surface exists in the shell: roster, create/select/delete, visibility, grid
  layout, drag/resize persistence, and built-in chart/stat/gauge widgets.
- Built-in widgets bind to real series either by explicit series name or by tag query. They backfill via
  store reads and receive live samples through the series SSE stream.
- Extension manifests may declare several `[[widget]]` tiles. Those tiles persist on the `Install`,
  are narrowed to the approved grant, and surface in `ext.list`.

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
- The **reusable source-picker package** (`@nube/source-picker`) — extract the shipped picker (db /
  datasources / Zenoh / flows / extension widgets) so surfaces OUTSIDE the dashboard reuse it — is
  **scoped, not built**: [`source-picker-package-scope.md`](source-picker-package-scope.md). Dashboard
  refactors onto it first (parity), then `thecrew` consumes it.

## Authoring rule

Keep new docs in this directory focused on dashboard scope. When a slice ships, promote the stable facts
to [`public/frontend/dashboard.md`](../../../public/frontend/dashboard.md) and leave session-specific
debugging and command output in `docs/sessions/`.
