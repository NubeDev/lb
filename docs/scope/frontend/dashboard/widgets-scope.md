# Frontend dashboard scope - widgets

Status: scope plus shipped-state reconciliation. Promotes to
[`public/frontend/dashboard.md`](../../../public/frontend/dashboard.md) as durable behavior. This doc
narrows the original [`dashboard-scope.md`](../dashboard-scope.md) and
[`dashboard-widgets-scope.md`](../dashboard-widgets-scope.md) to the widget surface.

Dashboard widgets are the small, repeatable unit inside a persisted dashboard grid cell. A cell stores
geometry, a `widget_type`, a data `binding`, and widget `options`; the host persists the cell in the
workspace-scoped `dashboard:{id}` record. Built-in widgets render today. Extension widget tiles are
declared and discoverable today. Rendering a federated extension widget inside a cell is the remaining
follow-up.

## Goals

- Keep the widget contract small and durable: `widget_type`, `binding`, and `options` in each dashboard
  cell.
- Render the Phase 1 built-ins: `chart`, `stat`, and `gauge`.
- Bind every widget to real series data, either by explicit series id or by tag-facet lookup through
  `series.find`.
- Separate state from motion: backfill history from `series.read`, read latest state from the store, and
  fold live samples from the series SSE stream.
- Surface installed extension widget tiles from `[[widget]]` manifest blocks through `ext.list`, with
  each tile scope narrowed to the admin-approved grant.
- Make the current unshipped line explicit: extension widget tiles are available as metadata, but
  `ext:<id>` cells are not mounted by `WidgetHost` yet.

## Non-goals

- No direct database access from widget code. Widgets read through host-gated series verbs.
- No write-capable widgets. A cell widget is read-only; action surfaces belong in full extension pages.
- No fake series data. Tests and demos seed real records through the real ingest path.
- No new persistence layer. Dashboard cells live inside the dashboard SurrealDB record; extension widget
  declarations live on the existing `Install` record.
- No claim that federated widget rendering has shipped until `WidgetHost` mounts `ext:<id>` through the
  bridge.

## Intent / approach

The dashboard keeps widgets deliberately constrained: one cell, one binding, one renderer, read-only
series access. That keeps the first-party widgets simple and gives the extension path a narrow trust
boundary.

The current built-in path is:

1. `AddWidget` creates a cell with `widget_type` of `chart`, `stat`, or `gauge`.
2. The user supplies either `{ series: "cooler.temp" }` or `{ find: { tags: [...] } }`.
3. `dashboard.save` persists the updated `cells[]` in the dashboard record.
4. `WidgetHost` dispatches the cell to the matching built-in widget.
5. `useSeries` resolves the binding, reads real samples, and opens the live series stream.
6. The widget renders loading, empty, denied, or value states honestly.

The extension path keeps the same cell contract. A manifest may declare one or more `[[widget]]` blocks.
Install projection turns those into `ExtUi` rows, narrows `scope` against approved caps, and exposes them
through `ext.list`. The next slice adds the palette integration and `WidgetHost` renderer for
`widget_type: "ext:<id>"`.

Rejected alternatives:

- **Let widgets query SurrealDB directly.** Rejected because widgets are the weakest principal and must
  never hold a DB handle or session token.
- **Poll `series.latest` for live values.** Rejected because live values are motion; the series stream is
  the right transport.
- **Treat extension widget metadata as rendered widgets.** Rejected because declaration and mounting are
  separate milestones. The docs and UI must not imply a tile renders until the renderer exists.

## How it fits the core

- **Tenancy / isolation:** the dashboard record is workspace-scoped, and series access derives workspace
  from the session token. A widget cannot name a foreign workspace.
- **Capabilities:** dashboard edits require `mcp:dashboard.save:call`; widget data reads require the
  series read caps. Extension widget scopes are intersected with the admin-approved grant at install and
  checked again by the host bridge.
- **Placement:** the same dashboard UI runs against the browser gateway path and the Tauri IPC seam.
  Transport changes, not the widget contract.
- **MCP surface:** widgets consume existing read verbs: `series.find`, `series.read`, `series.latest`, and
  the live series stream. They do not add write verbs.
- **Data:** built-in widget cells live in `dashboard:{id}.cells[]`; extension widget declarations live on
  `Install.widgets`.
- **Bus:** live samples come from the workspace-scoped series motion subject and are exposed to the
  browser through SSE.
- **Sync / authority:** dashboard cells are durable state and sync as dashboard records; live samples are
  best-effort motion.
- **Secrets:** widgets receive no secret material and no session token.
- **SDK/WIT impact:** the manifest contract is `[[widget]]` with versioned fields from
  `dashboard-widgets-scope.md`; the current implementation uses the TOML array-of-tables form and
  carries several tiles per extension.

## Example flow

1. An operator creates a dashboard named `Ops`.
2. They add a `chart` widget bound to `cooler.temp`.
3. The shell saves a new cell in `dashboard:ops`.
4. The chart backfills real samples through `series.read`.
5. The chart opens the series stream and folds each live `sample` event into its tail.
6. A teammate without the required series cap opens the same shared dashboard and sees a denied cell
   state, not a fabricated or leaked value.
7. Separately, the `fleet-monitor` extension declares two `[[widget]]` tiles. They surface in `ext.list`
   after install, but they are not mounted in the dashboard grid until the `ext:<id>` renderer ships.

## Testing plan

- **Capability deny-tests:** dashboard add/save/delete/share deny paths; extension bridge rejects and
  host-denies out-of-scope tools.
- **Workspace isolation:** dashboards and widget series reads do not cross workspaces.
- **Real-data UI tests:** create a dashboard, add a built-in widget bound to seeded real series, verify it
  renders and persists after reload.
- **Tag binding:** add a stat widget with a tag binding and verify it resolves through `series.find`.
- **Live motion:** gateway SSE test proves a real published sample reaches the stream.
- **Extension widget metadata:** install an extension with multiple `[[widget]]` blocks and verify both
  tiles round-trip through `ext.list`.

## Risks & hard problems

- The bridge boundary must stay narrow. Adding arbitrary tool calls to widgets turns a safe read-only
  cell into a privileged extension surface.
- Declaration and rendering are easy to conflate. Keep docs, tests, and UI labels precise until
  federated cell mounting ships.
- Series access must stay per-viewer. Sharing a dashboard must never widen the series grants behind a
  widget.
- The first extension widget renderer must prove teardown: unmount closes streams, uninstall removes the
  renderer, and no durable state stays inside the widget instance.

## Open questions

- What is the exact `widget_type` key for a declared extension tile: `ext:<extension-id>` or
  `ext:<extension-id>/<widget-label-or-id>`? Multiple `[[widget]]` blocks mean the cell key must identify
  a specific tile, not only an extension.
- Does the next slice add a dedicated widget expose, for example `./widget`, or reuse the existing page
  remote entry with a named exported mount?
- Should widget palette entries appear only to dashboard editors with `mcp:dashboard.save:call`, or also
  read-only users as disabled discoverable tiles?
- Do federated widgets get `series.watch` immediately, or start with `series.read`/`series.latest` and add
  streaming after teardown tests are in place?

## Related

- [`../dashboard-scope.md`](../dashboard-scope.md)
- [`../dashboard-widgets-scope.md`](../dashboard-widgets-scope.md)
- [`../../extensions/ui-federation-scope.md`](../../extensions/ui-federation-scope.md)
- [`../../../sessions/frontend/dashboard-session.md`](../../../sessions/frontend/dashboard-session.md)
- [`../../../sessions/extensions/fleet-monitor-federation-session.md`](../../../sessions/extensions/fleet-monitor-federation-session.md)
- [`../../../public/frontend/dashboard.md`](../../../public/frontend/dashboard.md)
