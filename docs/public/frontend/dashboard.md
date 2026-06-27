# Dashboard

The shipped dashboard surface is a workspace-scoped grid of widgets over real series data. A user can
list, create, open, edit, share, and delete dashboards through the shell. Layout edits persist in the
store, widgets read real series, and live values arrive over the series stream.

## What exists

- **Dashboard records:** `dashboard:{id}` records hold title, owner, visibility, and `cells[]`. A cell is
  react-grid-layout geometry plus `widget_type`, `binding`, and `options`.
- **Host verbs and routes:** `dashboard.list`, `dashboard.get`, `dashboard.save`, `dashboard.delete`, and
  `dashboard.share` are exposed through the host and mirrored by gateway routes.
- **Sharing:** dashboards use the S4 asset-sharing model: private, team, and workspace visibility.
  Workspace is the hard wall; capability and membership checks still apply per call.
- **Built-in widgets:** `chart`, `stat`, and `gauge` render in dashboard grid cells today.
- **Real bindings:** a widget binds either to an explicit series or to tags resolved by `series.find`.
- **Live data:** widgets backfill with `series.read` and fold live samples from
  `GET /series/{series}/stream`.
- **Seeded demo data:** `seed_iot_demo` writes real `cooler.temp` and `fryer.state` samples through the
  ingest path and tags them for dashboard tests and demos.
- **Extension widget declarations:** extensions may declare multiple `[[widget]]` tiles. Those tiles are
  persisted on the install, narrowed to the approved grant, and surfaced through `ext.list`.

## Widget behavior

Built-in widgets share the same data hook:

- resolve `{ series }` directly or resolve `{ find: { tags } }` through `series.find`;
- read recent samples through the real series read path;
- open the live series SSE stream when available;
- render loading, empty, denied, and value states honestly.

The current built-ins are:

| Type | Use | Data behavior |
|---|---|---|
| `chart` | Time-series line over numeric samples | Backfills recent samples and appends live samples. |
| `stat` | Latest value | Shows numeric payloads with optional unit, or a string payload as-is. |
| `gauge` | Latest numeric value against a min/max range | Reads `min`, `max`, and `unit` from cell options. |

## The widget contract, v2 (tool-driven builder)

The widget binding is now **generalized**: a cell binds a *view* to an *MCP tool call* — any tool in
the install grant, **read or write** — superseding the v1 read-only/four-series-verb contract. v1 cells
keep working (a v1 cell is a v2 cell whose tool set is the four series read verbs); every v2 cell, manifest
block, and bridge message carries a `v` field.

- **Cell v2 fields:** `view`, `source { tool, args }`, and (for controls) `action { tool, args_template }`,
  all serde-defaulted. `view` is the render vocabulary; `source` is the read/stream tool; `action` is the
  control's write tool.
- **The view vocabulary:** read views `chart`/`stat`/`gauge`/`table`; scripted views `plot` (Observable
  Plot), `d3`, `template` (JSX) — author code rendered in a **sandboxed iframe**, which **may write** a
  granted tool; control views `switch`/`slider`/`button` that call a write tool; and `ext:<id>/<widget>`
  extension tiles.
- **The bridge, v2:** `mount(el, ctx, bridge)` unchanged; `bridge.call(tool, args)` forwards any tool in
  `cell.tools ∩ install-grant` (the host re-checks the cap + workspace on every call); `bridge.watch(tool,
  args, onEvent)` streams `series.watch`/`bus.watch` over the shipped series SSE. **No token reaches the
  widget** — read or write, in-process or iframe.
- **The source picker:** the builder's left rail maps friendly labels (Series / Live-Zenoh /
  installed-extension / Action) to `{tool,args}` over the shipped `series.list`/`ext.list` — the author
  never types an MCP tool name.
- **Durable scripted templates:** a scripted snippet larger than ~4 KB persists as a workspace-scoped,
  author-owned `render_template:{id}` row via `template.save`/`get`/`list`/`delete` (gated
  `mcp:template.<verb>:call`); smaller snippets live inline in `cell.options.code`. Code is state →
  SurrealDB, never `localStorage`.
- **Trust tiers:** an allow-listed publisher key federates a widget in-process; everything else and all
  scripted views render in an opaque-origin iframe (`sandbox="allow-scripts"`, CSP, postMessage bridge).
  The allow-list defaults empty — in-process is opt-in.

The reference extension `proof-panel` ships a `[[widget]]` tile via a second `mountWidget` export on the
same remote — the model for an extension-shipped widget.

## Authorization

Dashboard access has three gates:

1. Workspace namespace from the session token.
2. Dashboard capability, such as `mcp:dashboard.list:call` or `mcp:dashboard.save:call`.
3. Dashboard visibility or membership: owner-private, team-shared, or workspace-visible.

Widget data access is separate. Sharing a dashboard does not widen series access. A viewer without the
needed series read grant sees a denied widget state rather than leaked or fake data.

Extension widget declarations add one more gate: the widget's declared `scope` is intersected with the
admin-approved install grant, and bridge calls are host-checked again.

## Tests

The shipped tests cover dashboard CRUD, per-verb denial, team-shared member/non-member behavior,
workspace isolation, seed integrity, gateway routes, live series streaming, built-in widget rendering
against a real gateway, tag-bound widgets, persistence after reload, and multi-`[[widget]]` extension
metadata round-tripping through `ext.list`.

## Follow-ups

- ~~Mount federated extension widgets in dashboard cells.~~ **Shipped** (`ext:<id>/<widget>` renderer).
- ~~Define the per-widget cell key for multi-widget extensions.~~ **Shipped** (`ext:<id>/<widget-id>`).
- ~~Add the untrusted iframe widget tier.~~ **Shipped** (opaque-origin sandbox + postMessage bridge).
- Add a multiplexed series stream for dashboards with many live widgets (each `watch` opens its own SSE).
- Add paged dashboard rosters and multi-editor live layout refresh.
- Generate shadcn `Select`/`Textarea` primitives so the builder's picker/code-editor drop the native
  elements (currently justified `eslint-disable`d).
- A `store.query`-style read tool as just another source the picker can name (no dashboard change needed).

## Related

- Scope index: [`../../scope/frontend/dashboard/README.md`](../../scope/frontend/dashboard/README.md)
- Widget scope: [`../../scope/frontend/dashboard/widgets-scope.md`](../../scope/frontend/dashboard/widgets-scope.md)
- Widget-builder (v2) scope: [`../../scope/frontend/dashboard/widget-builder-scope.md`](../../scope/frontend/dashboard/widget-builder-scope.md)
- Phase 1 session: [`../../sessions/frontend/dashboard-session.md`](../../sessions/frontend/dashboard-session.md)
- Widget-builder (v2) session: [`../../sessions/frontend/widget-builder-session.md`](../../sessions/frontend/widget-builder-session.md)
- Federation session: [`../../sessions/extensions/fleet-monitor-federation-session.md`](../../sessions/extensions/fleet-monitor-federation-session.md)
