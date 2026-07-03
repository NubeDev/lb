# Dashboard

The shipped dashboard surface is a workspace-scoped grid of widgets over real series data. A user can
list, create, open, edit, rename, share, and delete dashboards through the shell. Layout edits persist in the
store, widgets read real series, and live values arrive over the series stream.

## What exists

- **Dashboard records:** `dashboard:{id}` records hold title, owner, visibility, and `cells[]`. A cell is
  react-grid-layout geometry plus `widget_type`, `binding`, and `options`.
- **Host verbs and routes:** `dashboard.list`, `dashboard.get`, `dashboard.save`, `dashboard.delete`, and
  `dashboard.share` are exposed through the host and mirrored by gateway routes.
- **Roster management:** the left roster lists every reachable dashboard and, for editors
  (`mcp:dashboard.save:call`), exposes per-item **rename** (inline title edit → a title-only
  `dashboard.save` that preserves cells + variables) and **delete** (routed through the shared
  `ConfirmDestructive` gate → `dashboard.delete`; the host re-checks owner + capability). Rename is a
  title-only save on the same `dashboard:{id}` record — the id and layout never change.
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

## Flow⇄dashboard binding (pick a flow node + port — switch / slider / JSON, both ways)

A **Flows** binding makes a flow node port authorable in clicks (no MCP, no magic strings), reachable in
the **one PanelEditor** ("Edit panel"): pick **Datasource → Flows**, then a **flow node port** from the
`FlowsQuerySection`; the viz picker swaps to the control set (Switch/Slider/JSON) for an input port or the
JSON read view for an output port. It is built from shipped reads only (`flows.list` → `flows.get` →
`flows.nodes` descriptors) and is **agnostic to the node type + port names** — a node type a developer
ships tomorrow, with any `inputs[]`/`outputs[]` (not just `payload`), appears in the picker and drives /
reads back with **zero picker or engine changes** (the picker iterates descriptor ports; inject +
node_state read-back key on the `{node}:{port}` string; the read view extracts the *selected* port name):

- **Drill flow → node → port.** An **input port** resolves to a write `Action {tool:"flows.inject",
  argsTemplate:{id,node,port,value:"{{value}}"}}`; an **output port** to a read `Source
  {tool:"flows.node_state", args:{id,__flowNode,__flowPort}}`. The author sees friendly labels like
  `Cooler Control › setpoint-in › payload (input)` — never a tool name. The picker offers only flows the
  caller can `flows.get` (the cap-scoped offer; a flow the caller can't read never appears).
- **Switch / slider / JSON controls drive a port.** A switch sets a boolean `payload`; a slider a number
  (`options.min/max/step`); the **JSON control** (`JsonControl`) sends a structured `payload`, validating
  with ajv against the port's descriptor schema when one exists (`options.schema`), else free JSON — and
  **rejects invalid JSON before any call** (no fake accept). All three are one binding (`flows.inject` to
  a port) with different editors; the write goes through the leashed `WidgetBridge`, re-checked at the host.
- **Controls reflect the node's real current value.** On mount a flow-bound control reads its OWN retained
  input from the extended `flows.node_state` (per-port value wins over node-level), so a switch/slider/JSON
  shows true state after reload or restart — not a default. It advances on the canvas-cadence refresh tick
  (`useFlowNodeValue`); a `flows.node.watch` SSE is the later live-upgrade slice (never a `runs.get` poll,
  never a `series.watch` on an arbitrary node).
- **JSON / object read view (`jsonview`).** Pretty-prints a flow node's structured `payload` back out
  (collapsible) via `flows.node_state`; defaults to the `payload` field, `options.envelope` shows the
  whole `{payload, topic, …}` envelope. The one previously-missing read view (built ones:
  chart/stat/gauge/table/scripted/control). Both `json` and `jsonview` are registered in `WidgetView`.
- **Visual JSON-path builder (parse out the JSON).** Once an output node is bound, the
  `FlowsQuerySection` shows the node's CURRENT value as an interactive, collapsible **tree** (`JsonPathPicker`
  + `jsonPaths.ts`): objects → keys, arrays → indices, nested → deeper, scalars → leaves. Clicking a row
  binds **exactly that path** (e.g. `payload`, `payload.cron_ts`, `items[0].name`), stored on the source
  args as `__flowPath`; a live preview shows the resolved value. The picked path then feeds **any** view —
  not just the JSON view: `usePanelData` resolves a `flows.node_state` source CLIENT-SIDE through the
  path extraction and shapes it (a scalar → stat/gauge/text; an array → table/timeseries rows; an object →
  one row / the JSON view), so a flow value never lands as the raw whole-flow dump. Agnostic to shape and
  node type; a missing path resolves to `null` (honest), never a stale value.

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

## Grafana-compatible visualization — Phase 1 (`timeseries` end to end)

The viz slice adopts Grafana's panel model as an **additive superset** of the shipped v2 cell. Phase 1
ships the spine + the `timeseries` panel + the one editor. No v1/v2 cell breaks; every new field is
serde-default; no new datastore, no new verb (it rides `dashboard.save`/`get`).

- **The additive v3 cell shape.** `Cell` gains (all optional/serde-default): `description`, `sources[]`
  (Grafana *targets* — each `{ refId, datasource, tool, args, hide }`, superseding the single `source`;
  a v2 single-`source` cell reads as `sources[0]` via the `cellSources` adapter), `transformations[]`
  (**config only** in Phase 1 — the pipeline runs in the backend `viz.query`/`lb-viz` in Phase 3, never
  client-side), `fieldConfig` (per-field option defaults + overrides), and `pluginVersion`. `Dashboard`
  gains `schemaVersion` (our panel-model **document** version, pinned `=3` at save — distinct from
  `Cell.v`, the cell *contract* version, and not Grafana's interchange `schemaVersion`). The host stores
  `fieldConfig`/`transformations` opaquely (the UI owns the typed shape) and **bounds** the record:
  ≤32 transforms, ≤64 overrides/mappings/threshold-steps — an over-cap `save` is rejected server-side.
- **The `view` vocabulary adopts Grafana panel-type ids.** `timeseries`, `barchart`, `stat`, `gauge`,
  `bargauge`, `table`, `piechart`, … The shipped views are **aliases** (`chart` → `timeseries`); a v2
  `chart` cell renders unchanged through the new `timeseries` renderer. New cells write the canonical id;
  `canonicalView` resolves aliases at render.
- **The `timeseries` renderer — the full Grafana option surface.** Per-viz `options` (legend
  `showLegend`/`displayMode` list|table|hidden / `placement` / `calcs`; tooltip `mode`/`sort`) and
  per-field draw options in `fieldConfig.custom` (`drawStyle` line|bars|points, `lineWidth`,
  `fillOpacity`, …) — names taken verbatim from Grafana so import maps 1:1. Replaces the bad
  single-`unit`-string options.
- **The fieldConfig render path, through ONE user-prefs bridge.** `unit`/`decimals`/`min`/`max`/
  `thresholds`/`mappings`/`color`/`displayName`/`noValue`, plus `byName`/`byType` overrides. All value
  formatting goes through **one file** (`features/dashboard/fieldconfig/format.ts`) — never a hard-coded
  `toFixed` or unit string in a renderer. **Sequencing:** `lb-prefs` (`format.*`) is not shipped yet, so
  the bridge renders the documented **fallback** (canonical value + static unit label + local decimals)
  behind a `format.*`-shaped call site; when `lb-prefs` lands, the fallback is swapped for the real call
  **with no schema change and no re-save** (the `viaPrefs` flag is the swap-point guardrail). Thresholds
  **color** a value (and the line) — they never fire an alert (that's the rules engine). Grafana semantic
  color names resolve to `ui-standards` theme tokens; explicit hex passes through.
- **One data hook.** All panel data flows through `usePanelData` (Phase 1: the primary target over the v2
  bridge; Phase 3: swapped to `viz.query` in one file). No scattered `bridge.call` in renderers/editor.
- **One panel editor — add ≡ edit.** A single `PanelEditor` (a shadcn Sheet) mounts on a cell for both
  **Add panel** (a fresh default cell) and **Edit** (the ⚙ on a cell) — the same component, the same
  code path. It has the Grafana tab structure from day one (Query / Transform / Panel options / Field /
  Overrides) + a live preview + a viz picker + options search, reusing the shipped source picker, SQL
  Builder⇄Code editor, RefreshControl, and the `WidgetView`/`WidgetHost` render dispatch. The headline:
  **one pure `cell ↔ editorState` (de)serializer** (`cellToEditorState`/`editorStateToCell`) with the
  pinned identity `editorStateToCell(cellToEditorState(c)) ≡ c` for v1/v2/v3 cells — so reopening the
  editor reconstructs **every** option (including the SQL **Builder** state in `options.sql`), and add
  and edit can never drift. This makes the previously-false "add and edit share one field-code path"
  claim **true**, and fixes the reported "editing loses my SQL options" bug. The retired `WidgetBuilder`
  add-bar and the `CellSettings` ⚙ drawer are gone from the dashboard path (one surface, by design).
- **Authorization unchanged.** The editor is gated on `mcp:dashboard.save:call` (no editor entry point
  without it); the host re-checks `dashboard.save` on save (the authoritative backstop); per-target reads
  reuse the target tool's cap ∩ grant. Workspace isolation holds for v3 panels.

Tested against the real gateway/store (no mocks): the **ADD ≡ EDIT parity** headline (build → save →
reopen → every option identical, incl. the SQL Builder state), the `cell ↔ editorState` round-trip
(v1/v2/v3), backward-compat (a v1 series cell and a v2 chart+store.query cell load/render/re-save
unchanged), the format-bridge "no stored formatted string" assertion, live preview over real seeded rows
(honest deny when the source is denied), the edit-cap host save-deny backstop, and workspace isolation.

Phases 2–4 (the rest of the chart set, the backend `viz.query`/`lb-viz` transform pipeline + multi-
datasource targets, and Grafana JSON import/export) are scoped follow-ups on this same spine.

## Grafana-compatible visualization — Phase 2 (the rest of the everyday chart set)

Phase 2 fills in the standard chart vocabulary on the **same v3 spine** — six new panel renderers wired
end to end, each with its typed per-viz `options` (Grafana names + defaults verbatim) and the fieldConfig
render path through the **one** user-prefs bridge. **No backend change** (the host already stores
`fieldConfig`/`options` opaquely and bounds the record); **no new datastore, no new render capability, no
client-side transform** (invariant B holds — there are still no transforms; the pipeline is born backend
in Phase 3). All panel data still flows through the one `usePanelData` hook (invariant A).

- **Six new views: `stat`, `gauge`, `bargauge`, `table`, `barchart`, `piechart`** — one renderer file per
  view under `features/dashboard/views/<type>/`, recharts-based (extending the shipped SVG helpers; no
  visx — that's Phase 3). The shipped v2 `stat`/`gauge`/`table` views are **retired and replaced** by the
  new panels (a v2 cell renders through the new renderer unchanged — the canonical id is itself).
- **Typed per-viz `options`, Grafana-verbatim.** Each view has its own `options.ts` mirroring
  `timeseries`: `stat` (graphMode/colorMode/justifyMode/textMode/orientation + reduceOptions), `gauge`
  (showThresholdMarkers/Labels/orientation/reduceOptions), `bargauge` (displayMode basic|lcd|gradient /
  valueMode/showUnfilled/orientation + reduceOptions), `table` (showHeader/cellHeight/sortBy/pagination),
  `barchart` (orientation/stacking/showValue/barWidth/groupWidth + legend/tooltip), `piechart` (pieType
  pie|donut / displayLabels/legend/tooltip + reduceOptions). Names + defaults copied from
  `/tmp/grafana/public/app/plugins/panel/<type>/panelcfg.cue`.
- **`reduceOptions` — the one shared frame→value bridge.** `views/reduce.ts` owns
  `reduceFrame`/`reduceFrameValues`/`frameCategories` + the calc set (shared with the timeseries legend).
  It is the **explicit, visible** collapse of a frame to the single value(s) a single-stat panel draws
  (stat/gauge/bargauge/piechart) — never an implicit "guess a number", and **not** the transform pipeline.
  An empty/non-numeric frame reduces to `null` → an honest "no value", never a fabricated 0.
- **Per-field options via the existing bridge.** `views/field.ts` resolves the value field's effective
  `FieldOptions` + its threshold/fixed/palette color once; every value is formatted through
  `fieldconfig/format.ts` (unit/decimals) and colored through `fieldconfig/thresholds.ts` — no local
  `toFixed` or color string in any renderer. Thresholds **color**, never alert.
- **Result-shape ↔ type validation.** `views/shape.ts` classifies a target's rows
  (`scalar`/`series`/`table`/`unknown`, conservatively) and `usePanelShape` reads them through the one
  data hook; the **viz picker offers only the views a shape can honestly fill** (a scalar can't be a
  table; tabular rows can't be a gauge) — disabled with a reason, not hidden. `reduceOptions` is the
  visible bridge for the scalar/series → single-value collapse.
- **The editor extends, doesn't fork.** `editor/viewOptions.ts` adds the six defaults; `VizPicker` moves
  them from the disabled "Phase 2" list to buildable + shape-filters them; `tabs/PanelOptionsTab.tsx`
  becomes a thin dispatcher routing by canonical view to one per-view options editor under
  `tabs/options/` (the timeseries editor extracted there too). The add≡edit guarantee is unchanged —
  the new typed option keys are owned by the editor groups, so a fully-populated Phase-2 cell round-trips
  through the pinned `cell ↔ editorState` identity.

Tested against the real gateway/store (no mocks): **alias fidelity** (a seeded v2 stat/gauge/table cell
renders through the new renderer and re-saves identically), **options round-trip** (each view's typed
`options` survives `dashboard.save`/`get`), **result-shape↔type validation** over real seeded samples
(1-sample scalar offers stat/gauge not timeseries; multi-sample series offers timeseries + the single-stat
family; reduceOptions collapses a frame to one value), **fieldConfig through the one bridge** (a value
renders unit/decimals/threshold-color computed at render — no stored formatted string), and the mandatory
**capability-deny** (a denied target → honest denied state across stat/gauge/table, never a fake value) +
**workspace isolation**. Plus the extended pure `cellEditorState` round-trip (full stat/gauge/bargauge/
table/barchart/piechart cells) and `viz.phase2` reduce/shape unit tests.

Phase 4 (Grafana JSON import/export) remains the scoped follow-up on this spine. Deferred Phase-3 panels
(`histogram`, `state-timeline`, `status-history`, `heatmap`, `text` — the visx/markdown family) and the
named exotic panels degrade honestly on import.

## Grafana-compatible visualization — Phase 3 (backend-resolved transforms + datasource binding)

Phase 3 moves panel-data resolution into the backend and adds the transformation pipeline + multi-
datasource targets — **one implementation for every client** (web shell, React Native, email, webhook),
the same doctrine as `format.*`.

- **`lb-viz` — the pure transform lib (`rust/crates/viz/`).** The structural twin of `lb-prefs`: a pure
  Rust crate (no store/bus/I/O) compiled into every node, the ONE implementation of Grafana's transformer
  set over a canonical columnar `Frame` (`fields[]`, one typed column each). One transformer per file:
  `reduce`, `organize`, `filterFieldsByName`, `filterByValue`, `groupBy`, `joinByField`
  (`seriesToColumns`), `calculateField`, `sortBy`, `limit`, `merge`, `seriesToRows` — Grafana ids +
  option shapes **verbatim** (so a Phase-4 import is a near-identity). Empty/non-numeric input yields an
  honest result, **never a fabricated 0** (the no-mock rule, in the math). A `Matcher` (`byName`/`byType`/
  `byRegexp`/`byFrameRefID`) is mirrored in Rust here + TS for the editor.
- **`viz.query(panel) -> { frames, rows }` — the host resolver verb** (gated `mcp:viz.query:call`,
  member-level). For each non-hidden target in the panel's `sources[]` (falling back to the v2 `source`),
  it **re-enters the host MCP dispatcher** under the CALLER's principal + workspace — so each target tool's
  OWN cap and the workspace wall are re-checked, with **no render-path bypass**. A denied/failed target
  degrades to an **honest empty frame** (never a fabricated value, never a host-privilege read). It then
  assembles canonical frames, runs the `transformations[]` pipeline via `lb-viz`, and returns the frames
  (canonical) plus the primary frame flattened to `rows` — the SAME row shape the shipped renderers
  consume, so the swap changes nothing visible. The workspace comes from the **token**, never the panel.
  The cell still stores `transformations[]`/`fieldConfig` OPAQUELY; `viz.query` interprets them at run time
  (no record fork). A per-panel frame budget caps the assembled set.
- **Datasource binding.** A `DataSourceRef {type, uid}` selects the target's tool — native `surreal` →
  `store.query`, `series` → `series.*`, a registered `federation` source (`datasource:{ws}:{name}`) →
  `federation.query`. `viz.query` dispatches whichever tool the target names, leashed by that tool's cap +
  the workspace wall (a ws-B panel can never resolve a ws-A datasource — the federation gate is ws-pinned).
  No per-kind binding on the cell; no DSN at the panel.
- **The one-file client swap (invariant A).** `usePanelData`'s body now resolves a non-watch panel through
  `viz.query` (`builder/useVizQuery.ts`, debounced) returning the same `SourceState` — renderers + the
  editor preview are unchanged. A `series.watch`/`bus.watch` panel keeps the shipped live SSE path until
  the named `viz.stream` follow-up. Target args are interpolated against the resolved `VarScope` before the
  call (a `${host}` repoints the series exactly as before); the host also gets `scope` for transform
  options. No scattered `bridge.call` in any renderer (invariant A held); no client-side transform library
  (invariant B held — `views/reduce.ts` stays the per-viz frame→value reducer, NOT the pipeline).
- **The editor.** The Query tab gains a **datasource dropdown** (native + series + federation sources via
  `datasource.list`); a federation source uses the raw-SQL editor (the `federation.datasource.schema`
  column-dropdown verb is a named, deferred federation-plane follow-up). The Transform tab is now a **real
  pipeline editor** (add/reorder/disable/remove + per-id option fields) writing `transformations[]` config
  that `viz.query` runs — no client execution.
- **Tested (real infra, no mocks).** `lb-viz` units (49) cover each transformer incl. empty/non-numeric
  honesty. `crates/host/tests/viz_query_test.rs` (7) drives `viz.query` over a real node + store + caps: a
  store target + multi-transform pipeline returns the expected frames; **no-transform parity** with a
  direct `store.query`; a multi-target `joinByField` assembles; **`viz.query` deny without the cap**
  (opaque); a **denied target → honest empty, not a bypass**; **workspace isolation** (ws-B sees none of
  ws-A's rows); a **federation-bound target routes through `federation.query`** and an unregistered source
  resolves to an honest empty frame. Gateway: `viz.phase3.gateway.test.tsx` renders a seeded panel via
  `viz.query` identically to Phase 2 + authors a real pipeline. (`mcp:viz.query:call` was added to the dev
  session's `member_caps` — the new render path is member-level; see
  [`../../debugging/frontend/gateway-seed-series-500-denied-preexisting.md`](../../debugging/frontend/gateway-seed-series-500-denied-preexisting.md).)
- **Named follow-ups (deferred, not silent):** `viz.stream` (live frames over SSE, so live panels don't
  re-transform client-side); `federation.datasource.schema` (SQL-builder column dropdowns for an external
  source — a federation-plane add); and the `format.ts` → real `format.*` MCP swap (deferred: `formatValue`
  is synchronous at 13 render callsites, so the sync→async change is its own session — see
  [`../../sessions/frontend/format-prefs-swap-followup.md`](../../sessions/frontend/format-prefs-swap-followup.md)).

## Follow-ups

- **Grafana-compatible visualization (`viz/`) — Phases 1–2 shipped (above); Phases 3–4 scoped.** Phase 2
  shipped the rest of the everyday chart set (stat/gauge/bargauge/table/barchart/piechart). Phase 3:
  `viz.query` + `lb-viz` (swap `usePanelData`'s body), the
  transform pipeline, datasource binding beyond native SurrealDB. Phase 4: Grafana dashboard JSON
  import/export + `schemaVersion` migration. When `lb-prefs` ships, swap `fieldconfig/format.ts`'s
  fallback for the real `format.*` call (no schema change, no re-save). Scope:
  [`../../scope/frontend/dashboard/viz/README.md`](../../scope/frontend/dashboard/viz/README.md).
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

## X/Y plot builder (shipped 2026-07-01)

One chart model + renderer + builder, shared by the **dashboard panels** and the **in-channel query
results** — real titled X/Y axes, gridlines, a themed tooltip, a legend, and an interactive picker.

- **Model** `ui/src/lib/charts/`: a `PlotSpec` (`type`/`xField`/`yFields[]`/`seriesField?`/`stacked?`/
  `bins?`), field-kind inference (time/number/category by sampled values), `buildPlot` (rows+spec →
  Recharts frame: multi-series, long→wide pivot, pie aggregate, histogram bin), and `suggestPlot` (a TS
  twin of the host `pick_chart` so both surfaces open on the same default).
- **Renderer/builder** `ui/src/features/charts/`: `PlotChart` (real axes/ticks/grid, `ResponsiveContainer`,
  reduced-motion draw-in, empty/table-only states) and `PlotBuilder` (chart-type toggle + X/Y/series
  pickers + live preview).
- **Dashboard**: `timeseries`/`barchart`/`piechart` render through `PlotChart` when a `plot` spec is set
  (additive — no spec keeps the legacy chart). A **Plot** tab in the panel editor runs the draft query
  live and mounts the builder; the spec persists in `Cell.options.plot` via `dashboard.save`.
- **Channel**: run a query → "Customize" opens the builder; the viewer's choice persists **per-user**
  via the new host verbs `channel.chart_pref.get`/`.set` (record `channel_chart_pref:[ws, cid__item__user]`,
  gated by the channel `sub` cap) and is merged over the host's auto-pick. The canonical worker-authored
  result stays immutable — two viewers can plot it differently.

## Read cache & call de-duplication

Dashboard **reads** run through one `@tanstack/react-query` cache scoped to the visit
(`ui/src/features/dashboard/cache/`). A `DashboardCacheProvider` — mounted by `DashboardView` (and by the
channel `ResponseView`, which reuses the panel renderer) — mints a per-visit `QueryClient` and puts the
current `ws` in context; leaving the route drops the cache ("cache while here, clear on leave"). Every key
is **ws-prefixed** (no cross-workspace bleed; the host still re-derives the ws from the token) and
**canonicalised** (an unrelated edit doesn't re-key).

- **`viz.query`** — keyed on the resolved spec `{sources, transformations, fieldConfig, source, scope, tick}`,
  NOT the whole panel. The editor's probe/preview/plot consumers share one entry → one round-trip; a
  title/layout/option edit no longer refetches. The 200ms debounce is on the key input (one, not per-consumer).
- **Source picker + `datasource.list`** — one `["source-picker", ws]` bundle shared by the page and editor;
  `datasource.list` routes through one `["datasource.list", ws]` key (bundle + Query-tab dropdown share it).
  The package stays framework-light — a pure `loadSourcePicker(loaders)` in `@nube/source-picker` does the
  assembly; only the shell adapter wraps it in `useQuery`.
- **`flows.node_state`** — one whole-flow read per `(ws, flow, tick)`; N cells on one flow slice their own
  node/port client-side. **`series.read`** backfill is cached per binding; the live SSE tail stays outside
  the cache (state vs motion). Writes and streams are unchanged — this is a read-side layer only.

The de-dup, workspace-isolation, and deny behaviour is proven against the real gateway in
`cache/queryCache.gateway.test.tsx` (call counts instrumented on the `invoke` seam). SSE
subscriber-sharing is a deferred follow-up. Scope:
[`../../scope/frontend/dashboard-query-cache-scope.md`](../../scope/frontend/dashboard-query-cache-scope.md);
session: [`../../sessions/frontend/dashboard-query-cache-session.md`](../../sessions/frontend/dashboard-query-cache-session.md).

## Extension widgets over any source — frames-in (shipped 2026-07-03)

An extension `[[widget]]` tile can now be a **first-class visualization over the v3 panel model**, not
just a self-fetching tile calling its own tools. A `[[widget]]` that declares `data = true` opts into
frames-in: an `ext:<id>/<widget>` cell carries the SAME `sources[]` + `fieldConfig` + `transformations[]`
as a built-in `timeseries` cell, and the **shell** resolves them through the shipped `viz.query` path
under the **viewer's** grant and hands the tile **resolved frames**. The tile renders; it never fetches,
needs no read caps, and never sees a token or the DB.

- **Manifest opt-in.** `data = true` on `[[widget]]` (default `false`) projects through the existing
  Install→ExtUi path (`Widget.data` → `ExtUi.data`, `ui_decl::narrow`; a page is always `false`). A v2
  tile without it behaves exactly as before.
- **ctx v3 (additive).** The widget mount ctx gains `v: 3`, `data: Frame[]` (the `lb-viz` frame shape
  the built-in renderers consume), and `fieldConfig`; `mountWidget` MAY return `{ update?, teardown? }`
  instead of a bare teardown. On a data/vars/range tick the shell calls `update(ctx)` with fresh frames
  **in place — no re-mount** (the hard-won ExtWidget StrictMode per-run-slot lifecycle is preserved). A
  v2 tile (bare-fn return, no `data`) is byte-identical under the v3 shell. The contract's three mirrors
  (host `federationWidget.ts`, the devkit `src_contract.ts.tmpl` template, the extension copy) moved
  together in one slice; version-gate on `ctx.v`.
- **Shared resolution.** `useVizFrames` resolves a data cell's `sources[]` with the SAME bridge leash,
  interpolation, and `vizQueryKey` cache key as `useVizQuery`, so an ext data tile and a built-in bound
  to the same spec **share one gateway round-trip** (no per-tile duplicate stream).
- **Editor.** Picking a `data = true` widget in the Query tab's "Extension widgets" group KEEPS the
  cell's `sources[]` and shows the Query + Field tabs (reusing the built-in option registry verbatim) —
  the widget is the VIEW, the source is its binding, exactly like a built-in `timeseries`. A bare v2
  widget still owns its own data and clears targets.
- **Security unchanged.** The viewer's grant gates each source target (per-target deny in `viz.query`
  degrades to an honest empty frame, workspace-walled); the extension's grant is untouched — a data tile
  needs no new caps.
- **One render path across surfaces.** The same `ext:<id>/<widget>` data cell mounts through the ONE
  `WidgetView` dispatcher from a dashboard AND a channel `rich_result` (`ResponseView`), resolving
  identical frames on both.

The reference extension is **`echarts-panel`** (`rust/extensions/echarts-panel`): a `data = true` "Chart"
widget that renders `ctx.data` with Apache ECharts, mapping frames → an ECharts option driven by the
Field-tab options (units/decimals/thresholds/legend/axes) — no bespoke config UI, honest no-data/error
states, `{ update }` for in-place live re-render.

Proven against the real gateway in `builder/framesIn.gateway.test.tsx` (8/8): capability-deny,
workspace-isolation, v2-compat, frames-resolution, data-flag projection, and dashboard+channel parity.
Scope: [`../../scope/frontend/dashboard/ext-widget-source-binding-scope.md`](../../scope/frontend/dashboard/ext-widget-source-binding-scope.md);
session: [`../../sessions/frontend/ext-widget-frames-in-session.md`](../../sessions/frontend/ext-widget-frames-in-session.md).

## Library panels (reusable + standalone)

A **library panel** lifts the **non-layout half of a v3 `Cell`** into its own asset — a `panel:{id}`
record (the `dashboard` asset cloned one level down: slug id, owner, `private|team|workspace`
visibility, S4 `share` edge, tombstone delete, cap-gated verbs). It does two things an inline cell
cannot: many dashboards **reference** it (edit once, every referencing dashboard updates) and it renders
**standalone** on its own page, no dashboard grid.

- **The verbs** (`panel.get|list|save|delete|share|usage`, each its own file + cap
  `mcp:panel.<verb>:call`) mirror `dashboard.*` 1:1 — REST `/panels…` + the `POST /mcp/call` bridge,
  ws + owner from the token. `panel.list` is a cheap summary (id/title/view/visibility/updated_ts);
  `panel.usage` returns which dashboards reference a panel (delete-safety + the editor banner).
- **Ref cells + host-side hydration.** An additive `panelRef` on `Cell` makes a *ref cell*: layout + the
  ref + bounded overrides (title, `panelVars`), **no spec**. `dashboard.get` **hydrates** the spec from
  the panel record at read time under the viewer's gates; `dashboard.save` **validates** each ref
  resolves in-workspace (loud `BadInput`) and **strips the echoed spec** (the ref is authoritative — a
  stale hydrated copy a client re-sends can't de-link the cell). Inline and ref cells coexist by design.
- **Dangling / unreadable refs** degrade to an honest `panelMissing` placeholder in every host (grid,
  editor, standalone) — never a crash, never a leaked spec.
- **A lens over data access, never a grant.** Sharing a panel shares its *definition*; its `sources[]`
  re-check under the **viewer's** caps at render (`viz.query`'s per-target leash). A workspace-visible
  panel whose query needs a cap the viewer lacks renders "no data", not a leak.
- **Delete-safety.** `panel.delete` refuses while dashboards reference the panel (returns the usage
  list); `force=true` tombstones it and referencing cells show the placeholder until relinked; re-saving
  the panel un-hides them.
- **Editor affordances.** The panel editor gains **Save as library panel** (extract the spec → the cell
  becomes a ref), a **"Library panel — used on N dashboards"** banner (`panel.usage`) with **Save to
  library** (edits the shared record) and **Unlink** (copy the spec back inline — explicit drift), and
  the builder's **Add library panel** picker (`panel.list` → insert a ref cell).
- **The standalone page** `/t/$ws/panel/{id}` renders ONE panel full-bleed through the **same** shipped
  render path (`WidgetHost` → `WidgetView`/`usePanelData` → the viz bridge — no parallel renderer), with
  its own range picker + `?var-` selections, cap-gated on `panel.get`. This is the "chart not on a
  dashboard" surface, and a natural nav-entry target.

Proven against real infra: backend `crates/host/tests/panel_test.rs` (9/9 — the "sharing never widens
data access" + cross-ws `panel_ref` no-hydrate headlines, per-verb deny, ws-isolation, coexistence,
propagation, delete-safety); UI `features/panel/PanelPage.gateway.test.tsx` (8/8, real spawned gateway).
Scope: [`../../scope/frontend/dashboard/library-panels-scope.md`](../../scope/frontend/dashboard/library-panels-scope.md);
session: [`../../sessions/frontend/library-panels-session.md`](../../sessions/frontend/library-panels-session.md);
skill: [`../../skills/panels/SKILL.md`](../../skills/panels/SKILL.md).

## Related

- Scope index: [`../../scope/frontend/dashboard/README.md`](../../scope/frontend/dashboard/README.md)
- X/Y plot builder scope: [`../../scope/frontend/dashboard/viz/xy-plot-builder-scope.md`](../../scope/frontend/dashboard/viz/xy-plot-builder-scope.md)
- X/Y plot builder session: [`../../sessions/frontend/xy-plot-builder-session.md`](../../sessions/frontend/xy-plot-builder-session.md)
- Widget scope: [`../../scope/frontend/dashboard/widgets-scope.md`](../../scope/frontend/dashboard/widgets-scope.md)
- Widget-builder (v2) scope: [`../../scope/frontend/dashboard/widget-builder-scope.md`](../../scope/frontend/dashboard/widget-builder-scope.md)
- Phase 1 session: [`../../sessions/frontend/dashboard-session.md`](../../sessions/frontend/dashboard-session.md)
- Widget-builder (v2) session: [`../../sessions/frontend/widget-builder-session.md`](../../sessions/frontend/widget-builder-session.md)
- Widget-builder follow-ups (SQL source + editors) session: [`../../sessions/frontend/widget-builder-followups-session.md`](../../sessions/frontend/widget-builder-followups-session.md)
- Federation session: [`../../sessions/extensions/fleet-monitor-federation-session.md`](../../sessions/extensions/fleet-monitor-federation-session.md)
