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
- The external-data **reference extensions** whose tools/tiles a widget would call (timescale, mqtt-bridge)
  are blocked on separate platform fixes (native host-callback, `net:*`, `kv.*`, secrets) —
  [`../../extensions/reference-extensions-scope.md`](../../extensions/reference-extensions-scope.md). The
  dashboard is ready for them; they are not yet built.

## Authoring rule

Keep new docs in this directory focused on dashboard scope. When a slice ships, promote the stable facts
to [`public/frontend/dashboard.md`](../../../public/frontend/dashboard.md) and leave session-specific
debugging and command output in `docs/sessions/`.
