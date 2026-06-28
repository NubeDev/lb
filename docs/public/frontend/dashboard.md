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
- **Trust tiers:** an **installed extension widget** federates **in-process** (against the shell's React
  singleton) — installing an extension already passes the publish/install capability gate, so the install
  *is* the trust decision, and a federated remote externalizes React to the shell import map, which only
  exists in-process. **Scripted views** (`plot`/`d3`/`template`) — author code typed into a cell — render
  in an opaque-origin iframe (`sandbox="allow-scripts"`, CSP, postMessage bridge); that sandbox is for
  untrusted author code only, never an installed widget. (Earlier the iframe tier also tried to host
  non-allow-listed extension widgets; it couldn't — see
  [`../../debugging/frontend/ext-widget-iframe-tier-cannot-resolve-bare-react.md`](../../debugging/frontend/ext-widget-iframe-tier-cannot-resolve-bare-react.md).)

The reference extension `proof-panel` ships **two** `[[widget]]` tiles via one `mountWidget` export
(dispatched by `widgetId`) on the same remote — the model for an extension-shipped widget, and the proof
that one extension can ship N tiles:

- **Proof Ping** — reads `proof.demo`'s latest **once** (`bridge.call("series.latest")`; state, rule 3).
- **Proof Ping Live** — the **SSE example**: backfills with `series.latest`, then **subscribes** to
  motion via `bridge.watch("series.watch", {series:"proof.demo"})` → the shipped `openSeriesStream` → the
  gateway SSE `GET /series/{series}/stream` → the workspace motion subject, updating per live sample with
  no reload or polling. Its `[[widget]].scope` names `series.watch` (and the manifest requests
  `mcp:series.watch:call`, so `ui_decl::narrow` keeps it in the granted scope); the SSE endpoint itself
  authorizes on `series.read`. The stream tears down on unmount (stateless eviction). A live Playwright
  e2e writes a fresh sample and asserts the tile ticks to it in a real browser.

## Extension widgets in the palette (the last mile)

A packaged `[[widget]]` tile is now **addable from the builder**, not only renderable from a hand-authored
cell key:

- **A new "Extension widgets" picker group.** The builder's source picker emits **one entry per installed
  extension's `[[widget]]` tile** (`extWidgetEntries` over the shipped `ext.list.widgets[]`), labelled
  `<ext> · <tile.label>` (e.g. `proof-panel · Proof Ping`) and carrying the tile's icon. This is distinct
  from the extension's *tool* entries (build-your-own views) — a tile is a finished widget the developer
  shipped, a different author intent and a different cell shape.
- **Selecting a tile is a one-click placement.** A packaged tile *is its own view* — the view chooser is
  hidden, and selecting it produces a v2 cell `{ v:2, view:"ext:<id>/<widget>" }` (no `source`/`action`;
  the tile owns its data via its `scope ∩ grant`). The widget id is the renderer's own `widgetIdOf` slug,
  so the key the picker builds is exactly the key `ExtWidget` parses. Preview routes through the shipped
  `WidgetView → ExtWidget` over the real bridge, rendering the tile **in-process** (the install is the
  trust gate — see Trust tiers below).
- **The add affordance is gated to editors.** The whole "Add widget" surface renders only when the session
  holds `mcp:dashboard.save:call` for the active workspace (`canEdit`, sourced from the routing-context
  caps the shell already holds — the same source the nav gates editing on; no new backend read). A
  read-only viewer sees the dashboard with **no add surface**. The host re-check on `dashboard.save`
  remains the authoritative backstop — the UI gate is convenience, never the security boundary.

No backend, no v2 contract, no `mountWidget`/`[[widget]]` change — a frontend discovery-and-gating slice.

## Widget settings (edit a cell, not re-add)

A cell can now be **reconfigured after it lands** instead of being deleted and re-added:

- **A cell `title`.** `Cell` gains an additive `title` field (`#[serde(default)]` server-side,
  `Cell.title?` client-side) that round-trips through the existing `dashboard.save`/`get` — no new verb.
  The header renders the title, falling back to a derived label (`cellLabel`: source tool → action tool →
  view) when empty, so an untitled cell still reads honestly.
- **A per-cell ⚙ settings drawer.** In edit mode, each cell shows a ⚙ button (gated on
  `mcp:dashboard.save:call`, the same edit gate as the palette add surface) that opens a Sheet hosting the
  WidgetBuilder in an **edit-existing-cell** mode: the source/view/options/title are seeded from the cell
  (`seedEntryId` maps the cell back to its picker entry — packaged tile by view key, SQL by `store.query`,
  else read/action tool + series arg). Saving rebuilds the cell keeping its key + geometry and persists the
  **whole dashboard** via `saveCells`/`dashboard.save`. The server re-checks the cap on save regardless.
- **One authoring surface.** Edit mode reuses the exact builder fields (`seed`/`onSave`/`bare`), not a
  parallel editor — so add and edit share one set of field code and cannot drift.

## The shared vars library (`ui/src/lib/vars/`) — the frozen interpolation spine

A pure-TS module (no React, no `@/` shell imports) that the shell **and** federated extension remotes
link — a Grafana-style template-variable engine, frozen by `VARS_LIB_V`:

- **One model.** A `Variable` is a name bound to a resolver — `query`/`source` map to one `{tool,args}`,
  the static forms (`custom`/`text`/`const`/`interval`) carry their own value. The resolved selection +
  the built-ins form a `VarScope { values, builtins }`.
- **`interpolate(template, scope)`** handles the three reference syntaxes (`$var`, `${var}`, `[[var]]`),
  the format hints (`${var:json|csv|singlequote|doublequote|pipe|raw}`), and multi-value selections, and
  **leaves an unknown variable literal** (Grafana behavior — a shared link always renders, never throws).
- **`interpolateArgs(argsTree, scope, runtimeValue?)`** deep-substitutes a JSON value tree,
  **type-preserving**: a sole `${var}` reference returns the raw value (a multi-value becomes a real array
  for a JSON `IN` sink; a number/bool passes through). It generalizes the control `{{value}}` slot —
  `views/argsTemplate.ts` `fillArgs` now delegates to it, so there is one substitution engine.
- **`resolveBuiltins(inputs)`** is pure — the shell supplies `$__from/$__to/$__range*/$__interval*/
  ${__user.*}/${__dashboard}/${__workspace}/${__value}` from the verified token + the URL time range, never
  a cell or iframe. A missing input yields no key (the reference stays literal, not a fake empty), and an
  extension never resolves identity itself — it is handed resolved values in `ctx` (Slice 3).
- **`extractVarNames` / `extractVarNamesDeep`** give the refresh dependency set + the deny-set.

This is a forever boundary the moment an extension links it; the contract is `interpolate`/
`interpolateArgs` + `VarScope` + `resolveBuiltins`, versioned by `VARS_LIB_V`.

## Dashboard variables (Grafana-style)

A dashboard can define **variables** — a name bound to a resolver — and reference them across its cells:

- **Definitions on the record.** `Dashboard.variables[]` (additive `#[serde(default)]`, no new verb)
  holds each variable: a `query`/`source` variable resolves its options over a granted `{tool,args}`; a
  `custom`/`interval` variable carries a static list; `text`/`const` a single value. The host stores only
  the definitions — the per-viewer **selection lives in the URL**.
- **Selection in the URL.** Selected values are flat `?var-<name>=` params (repeated for multi-value),
  parsed by `validateDashboardSearch` (malformed degrades to defaults, never throws) and translated by
  `varsFromSearch`/`withVar`. A shared link carries the selection but not authority — the gateway
  re-derives the workspace from the token, so a URL var value can't cross the wall.
- **The variable bar.** A dropdown per variable (single / multi / include-all), a text input for `text`,
  hidden for `const`. Query/source options resolve over the **same leashed bridge** a cell uses
  (`makeWidgetBridge([tool])`, host re-checks the cap + workspace per call); a denied query is an honest
  empty list, never a fabricated catalogue.
- **The variable editor** (gated on the edit cap) adds / edits / reorders variables; a query/source
  variable picks its resolver via the **source picker** (the author never types a tool name).

## Variable interpolation into cells (+ ctx.vars / ctx.timeRange)

The shell resolves a `VarScope` (`useVarScope`: the URL selection + defaults + token/range-derived
built-ins) and threads it into every cell:

- **Every cell call is interpolated.** `useSource` runs `interpolateArgs(source.args, scope)` before the
  bridge call (and the watch args); a control runs `interpolateArgs(action.argsTemplate, scope, value)`.
  A cell re-points by variable (`series.read {series:"${host}"}` → the selected series). For a
  `store.query` source the substitution runs over the arg tree (the bound `vars`) — never string-spliced
  SQL; the host parse-allowlist is the boundary.
- **The widget ctx gains `vars` + `timeRange` (additive v2, `WIDGET_CTX_V`).** An extension tile is handed
  the resolved selections + the URL time range + the built-ins; a v1 widget that ignores them is
  unaffected. The shell resolves the scope from the verified token — the extension/iframe **never**
  resolves `${__user.*}`/`${__workspace}` itself (un-spoofable), and no token crosses the boundary.

## Auto-refresh + live events

- **A refresh picker** (`RefreshControl`, URL `?refresh=30s`; off/5s/10s/30s/1m/5m/15m). On each tick
  `useAutoRefresh` bumps a `refreshKey` that re-resolves query variables (`useVariableOptions`) and
  re-runs each read cell's source (`useSource` re-keys on it) — polling **state**. Pauses when the tab is
  hidden; in-flight dedupe is the re-keyed effect's job.
- **Live push** composes with refresh (motion vs state). The WidgetBridge `watch` routes `series.watch` to
  `/series/{s}/stream` and `bus.watch` to the new `/bus/stream?subject=` SSE (`openBusStream`); a cell folds
  pushed payloads in live. A cell declares which it uses — refresh polls state, watch streams motion.

## Generic bus pub/sub (bus.publish / bus.watch)

A shared platform surface (not dashboard-private): generic, workspace-walled, capability-gated subject
pub/sub, mirroring `ingest`/`series` (one verb per file):

- **`bus.publish(subject, payload) -> {ok}`** — fire-and-forget motion. NOT durable (rule 3): `{ok}` means
  "handed to the bus", never "delivered"; a must-deliver effect still goes through the outbox.
- **`bus.watch(subject) -> stream`** — subscribe to a walled subject.
- **The workspace wall is structural.** The caller's `subject` is namespaced to `ws/{id}/ext/{subject}`
  host-side from the token; reserved prefixes (`series/`, `channels/`, `internal/`, `ws/`, `presence/`)
  and escape attempts are refused. A caller can never name another workspace's subject nor impersonate
  platform motion.
- **Gated `mcp:bus.publish:call` / `mcp:bus.watch:call`**, opaque deny. Reachable via `POST /mcp/call`
  (`bus.publish`), `POST /bus/publish`, and `GET /bus/stream?subject=&token=` (the SSE feed, auth-first
  401/403 like the series stream). A widget reaches them only via `cell.tools ∩ grant`, re-checked host-side.

## JSON payload builder

A control cell can author a **JSON payload** sent to a write target on interaction:

- **`JsonPayloadField`** — a CodeMirror JSON editor authoring a template with `${var}`/`{{value}}` slots,
  a **target picker** (`bus.publish`, `ingest.write`, or an installed extension's write tools), and a
  subject input for `bus.publish`. On send: `JSON.parse` → `interpolateArgs(template, scope)`
  (type-preserving, the shared lib) → a leashed `makeWidgetBridge([target]).call(target, payload)`.
- **No fake delivery.** A `bus.publish` is fire-and-forget — the UI shows "published" (handed to the bus),
  never "delivered"; a must-deliver effect targets a tool that enqueues to the outbox. The target must be
  in the cell's tool set ∩ grant (bridge leash + host re-check).
- Lives in the ⚙ settings drawer for a control cell (button/switch/slider).

## Authorization

Dashboard access has three gates:

1. Workspace namespace from the session token.
2. Dashboard capability, such as `mcp:dashboard.list:call` or `mcp:dashboard.save:call`.
3. Dashboard visibility or membership: owner-private, team-shared, or workspace-visible.

Widget data access is separate. Sharing a dashboard does not widen series access. A viewer without the
needed series read grant sees a denied widget state rather than leaked or fake data.

Extension widget declarations add one more gate: the widget's declared `scope` is intersected with the
admin-approved install grant, and bridge calls are host-checked again.

## The "Direct SurrealDB" source + the in-app editors (widget-builder follow-ups A/B/C)

A SQL source and the authoring editors, additive over the v2 contract — a SQL source is just another
`{ tool, args }`; a code editor is just the authoring UI for the shipped `plot`/`d3`/`template` views.

- **`store.query` / `store.schema` (read-only SurrealDB).** Two host MCP verbs:
  - `store.query(sql, vars?) -> { columns, rows }`, gated `mcp:store.query:call`. **Read-only is
    enforced by PARSING** the statement (SurrealDB's own parser) and allowlisting by **kind** — a single
    `SELECT` (plus `INFO`/`SHOW`); `CREATE`/`UPDATE`/`UPSERT`/`DELETE`/`INSERT`/`RELATE`/`DEFINE`/
    `REMOVE`/multi-statement/`USE` are each refused **before** the SQL reaches the store. Never a
    substring check. Runs inside the caller's workspace namespace (from the token, never the SQL), bounded
    to 10k rows / 5s. Mutation goes through the typed write tools, never this verb.
  - `store.schema() -> { tables:[{name, columns:[{name,type}]}] }`, gated `mcp:store.schema:call`,
    workspace-walled — the visual SQL builder's dropdown source.
  - Both are reached over the **one bridge** (`POST /mcp/call`) like any tool, leashed by
    `cell.tools ∩ grant`; the source picker's "Direct SurrealDB" entry produces
    `{ tool: "store.query", args: { sql } }`, and every existing view renders its rows unchanged.
- **The in-app CodeMirror editors** (`@uiw/react-codemirror`): a JSX `CodeEditor`, a Plot/D3
  `PlotCodeField`, a `TemplateSourceField` (inline OR a saved `render_templates` pick via `template.list`
  over the bridge), and a raw-SQL `SqlEditor`. They author a code **string** into `cell.options.code` /
  a `render_template` reference — the string runs only in the sandboxed iframe (trust unchanged).
- **The Grafana-style Builder⇄Code SQL editor:** a typed `SqlBuilderQuery` (table, columns +
  aggregation, filters, group-by, order, limit) + a `toSurrealQL` renderer, with a Builder/Code toggle
  (confirm-on-switch-back). The cell stores **both** the raw string (what `store.query` runs) and the
  builder query (so reopening returns to the builder). Builder mode can only generate a `SELECT`; Code
  mode is still parse-allowlisted by `store.query`.

## Tests

The shipped tests cover dashboard CRUD, per-verb denial, team-shared member/non-member behavior,
workspace isolation, seed integrity, gateway routes, live series streaming, built-in widget rendering
against a real gateway, tag-bound widgets, persistence after reload, and multi-`[[widget]]` extension
metadata round-tripping through `ext.list`.

The extension-widget palette adds (real gateway, real installed `proof-panel`, no fake): one `widget`
entry per `[[widget]]` tile (unit + real `ext.list`); a full builder round-trip (palette lists `Proof
Ping` → select hides the view chooser → preview mounts the real `ExtWidget` over the bridge, its
`proof.demo` latest asserted live → **Add** persists a `view:"ext:proof-panel/proof-ping"` cell →
`getDashboard` re-reads it); the edit-cap gate (a `canEdit=false` viewer renders an empty add surface
**and** `dashboard.save` is denied server-side for a principal lacking the cap); workspace isolation (a
ws-B editor's picker lists only ws-B tiles); and trust-tier routing re-asserted from the palette path (an
installed tile renders **in-process**, never sandboxed). A **live Playwright e2e**
(`ui/e2e/dashboard-widget.spec.ts`, built shell + real node) adds the tile from the palette and asserts
it mounts in-process with the host's single React and renders the real `proof.demo` value — the failure
mode (`Failed to resolve module specifier "react"`) only shows in a real browser.

The SQL source + editors add: `store.query` deny / parse-rejection-per-write-kind / two-session
isolation / row-cap / SELECT round-trip and `store.schema` deny + isolation (real store, seeded via the
real ingest path); `toSurrealQL` unit cases + a Builder→Code→Builder round-trip; and an end-to-end
"build a query in the visual editor → Run → rows render in a table AND a chart widget" over the real
gateway.

## Follow-ups

- ~~Mount federated extension widgets in dashboard cells.~~ **Shipped** (`ext:<id>/<widget>` renderer).
- ~~Define the per-widget cell key for multi-widget extensions.~~ **Shipped** (`ext:<id>/<widget-id>`).
- ~~Add the untrusted iframe widget tier.~~ **Shipped** (opaque-origin sandbox + postMessage bridge).
- ~~Surface packaged `[[widget]]` tiles in the builder palette, gated to dashboard editors.~~ **Shipped**
  (the "Extension widgets" group + the `mcp:dashboard.save:call` edit gate;
  [`widget-palette-scope.md`](../../scope/frontend/dashboard/widget-palette-scope.md)).
- Show a read-only viewer a ghosted "ask an editor to add" tile instead of hiding the add surface entirely.
- Add a multiplexed series stream for dashboards with many live widgets (each `watch` opens its own SSE).
- Add paged dashboard rosters and multi-editor live layout refresh.
- Generate shadcn `Select`/`Textarea` primitives so the builder's picker drops the native `<select>`
  (the code editors now use CodeMirror; the picker still uses a justified `eslint-disable`d `<select>`).
- ~~A `store.query`-style read tool as just another source the picker can name.~~ **Shipped** (the
  "Direct SurrealDB" source — `store.query`/`store.schema`, parse-allowlisted + workspace-walled).
- ~~An in-app code editor for the scripted `plot`/`d3`/`template` views.~~ **Shipped** (CodeMirror).
- ~~A Grafana-style Builder⇄Code visual SQL query builder.~~ **Shipped** (`SqlBuilderQuery` + `toSurrealQL`).
- A SurrealQL CodeMirror grammar (the SQL editor currently uses the close-enough `@codemirror/lang-sql`).
- An MCP `sql.generate` tool to restore the (dropped) AI "generate SQL" button.
- A LogQL-style source (port the Grafana Loki builder/raw files, kept as the reference).

## Related

- Scope index: [`../../scope/frontend/dashboard/README.md`](../../scope/frontend/dashboard/README.md)
- Widget scope: [`../../scope/frontend/dashboard/widgets-scope.md`](../../scope/frontend/dashboard/widgets-scope.md)
- Widget-builder (v2) scope: [`../../scope/frontend/dashboard/widget-builder-scope.md`](../../scope/frontend/dashboard/widget-builder-scope.md)
- Phase 1 session: [`../../sessions/frontend/dashboard-session.md`](../../sessions/frontend/dashboard-session.md)
- Widget-builder (v2) session: [`../../sessions/frontend/widget-builder-session.md`](../../sessions/frontend/widget-builder-session.md)
- Widget-builder follow-ups (SQL source + editors) session: [`../../sessions/frontend/widget-builder-followups-session.md`](../../sessions/frontend/widget-builder-followups-session.md)
- Federation session: [`../../sessions/extensions/fleet-monitor-federation-session.md`](../../sessions/extensions/fleet-monitor-federation-session.md)
