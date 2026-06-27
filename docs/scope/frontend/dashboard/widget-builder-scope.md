# Frontend dashboard scope — the tool-driven widget builder (any MCP tool → any view)

Status: **SHIPPED** (2026-06-27) — promoted to [`public/frontend/dashboard.md`](../../../public/frontend/dashboard.md);
session [`widget-builder-session.md`](../../../sessions/frontend/widget-builder-session.md). Originally: Target stage: **S9+ collaboration UI**, **after** `dashboard-scope.md` Phase 1 (the grid +
cell record) and the shipped federated-page bridge (`proof-panel` proves it end to end). This scope is the
**generalization** of [`dashboard-widgets-scope.md`](../../dashboard-widgets-scope.md) and
[`widgets-scope.md`](widgets-scope.md): it widens the widget from *"a read-only renderer bound to one
series"* to **"any view bound to any MCP tool the install grant allows — read or write."**

We want a **widget builder** in the dashboard: a user picks a **data source that is just an MCP tool
call**, maps its result into a **view** (axis chart, table, stat, gauge, **Observable Plot**, **D3**, or a
**JSX render template**), or drops a **control** (switch / slider / button) that *calls* a tool, and saves
it as a dashboard cell. Widgets are authored three ways — **configured** in-app (no code), **scripted**
in-app (an inline Plot/D3/JSX template), or **shipped by an extension developer** (a `[[widget]]` tile,
modelled on `proof-panel`). Every path rides the **one shipped bridge** (`bridge.call(tool, args)`),
leashed by the install grant and re-checked at the host. This makes the dashboard a **generic front-end
for the MCP tool surface** — exactly rule 7.

---

## The headline reframe: a widget binds a *view* to an *MCP tool call*

Today's widget binding is `{ series } | { find: { tags } }` and the contract is "read-only, four series
verbs, never touch the DB, never write." That was over-cautious, and the **shipped code already disproves
it**: `proof-panel`'s federated page reaches data through `bridge.call(tool, args)` and its `[ui] scope`
already grants **write** verbs (`ingest.write`, `inbox.record`, `outbox.enqueue`, its own `proof.derive`).
The page bridge is a *general, grant-leashed tool forwarder*, not a four-verb read pipe. A widget is just
that bridge **narrowed to one cell** — so it inherits the same generality for free.

So the binding generalizes:

```
  Cell.source = { tool: "<mcp-tool>", args: {…} }      // ANY tool in the install grant — read OR write
  Cell.view   = "chart" | "stat" | "gauge" | "table"   // read views (render the tool's result)
              | "plot" | "d3" | "template"             // scripted views (code over the result rows)
              | "switch" | "slider" | "button"         // control views (a tool the cell CALLS)
```

| Your phrasing | Is just an MCP tool, no special-casing |
|---|---|
| "a series" | `series.read` / `series.latest` / `series.watch` |
| "direct SurrealDB" (via tools, not the backend) | a read-only `store.query`-style tool **if one exists** — otherwise just any read tool |
| "Zenoh as messages" | `series.watch` / a `bus.watch(subject)` streaming tool — host-mediated, never a raw Zenoh handle |
| "if I made an MQTT extension" | the extension's own `mqtt.subscribe` / `mqtt.status` / `mqtt.publish` |
| "a switch / slider" | a **control** view whose action is `mqtt.publish` / `zenoh.command` / any granted write tool |

The dashboard stops being an IoT series viewer and becomes a **front-end for whatever tools a workspace
has installed**. The IoT-ness, the MQTT-ness, the SurrealDB-ness all live in *which tool* a cell names —
core never learns "fryer" or "broker" (the ingest/vision rule holds).

---

## Goals

- **A widget builder** (`features/dashboard/builder/`), modelled on rubix-cube's `chart-builder`: pick a
  source tool, run it, introspect the result columns, map `x/y/breakdown`, choose a view, preview live,
  "Add to dashboard." The author **never types an MCP tool name** — a friendly **source picker** (series /
  Zenoh / "my MQTT extension" / …) hides the tool behind a label (see "The source picker" below).
- **The full view vocabulary**, ported from rubix-cube's `ChartType` and extended:
  - **Read views:** `chart` (line/bar/h-bar, the `time-series-chart`), `stat`, `gauge`, `table`
    (the `InfiniteDataTable` over rows).
  - **Scripted views:** `plot` (an [Observable Plot](https://observablehq.com/plot/) snippet over rows),
    `d3` (a [D3 / Observable](https://observablehq.com/) snippet), `template` (a JSX render template over
    rows). All three are **arbitrary code** → they render in the sandboxed-iframe tier (or trusted-key
    in-process), and **may write** (below).
  - **Control views:** `switch`, `slider`, `button` — a current-value read (optional) plus a **write tool**
    the control invokes on interaction.
- **Write-capable widgets, leashed by the grant.** A control and a scripted template may call **write**
  tools (`mqtt.publish`, `zenoh.command`, `ingest.write`, an ext's own verb) — but **only** tools in the
  cell's `source`/`actions` set ∩ the install grant, re-checked at the host per call. The grant is the
  leash, not a hardcoded read-only allowlist.
- **Widgets from extensions — first-class.** An extension developer ships widgets the same way
  `proof-panel` ships a page: `[[widget]]` tiles in `extension.toml` (`entry`, `label`, `icon`, `scope`),
  federated, installed per workspace, surfaced in the palette, mounted in a cell by trust tier. This is the
  **`ext:<id>/<widget>` renderer** that `widgets-scope.md` lists as the open follow-up — built here.
- **Three authoring tiers, one cell contract.** (1) configured built-in (no code), (2) inline scripted
  template (code in-app, saved as a `render_templates` row / inline), (3) packaged extension widget. All
  three produce the same `{ source, view, options }` cell and ride the same bridge.
- **One persisted contract.** The cell still lives in the workspace-scoped `dashboard:{id}.cells[]` record
  (no new persistence). A scripted template's code lives in a `render_templates` SurrealDB row (or inline
  in the cell `options`), never `localStorage`.

## Non-goals

- **No raw DB handle, no token, ever at the widget.** "Direct SurrealDB access" means *through a
  host-gated MCP tool*, never a store connection or a session token. The hard invariant of
  `dashboard-widgets-scope.md` that stays.
- **No un-leashed tools.** A widget can call only tools in its cell's declared set ∩ the install grant.
  Generalizing read→write does **not** mean "any tool" — it means "any *granted* tool." The deny path is
  the headline test.
- **No in-process untrusted code.** Scripted views (Plot/D3/JSX) and untrusted extension widgets render in
  a sandboxed iframe; in-process module federation is trusted-publisher-key-only (unchanged from
  `ui-federation-scope.md`). Letting a user-typed JSX snippet run in the shell process is the one thing we
  do not do.
- **No new datastore / compute plane.** Cells, dashboards, templates are SurrealDB records; aggregation is
  the source tool's job (or `series.read` shaping), not a new engine.
- **No `if cloud {…}`.** Same builder, two transports (Tauri `invoke` / gateway SSE+HTTP).
- **No `*.fake.ts`.** Real gateway, real installed reference widget, real tools, seeded real rows.

## Intent / approach

**Marry rubix-cube's view+builder layer onto lazybones's cell + bridge + capability model.** rubix-cube
already has the exact view vocabulary the user wants (`chart-builder/types.ts`: line/bar/table/**Template
(JSX, iframe)**/**Plot**); lazybones already has the durable cell record, the host-mediated bridge, and
the grant leash. The work is the join, not either half:

- **Copy nearly wholesale (frontend):** `chart-builder/` (the builder UX + its zustand store +
  `transformDataToColumns` column introspection), `charts/time-series-chart/`, the `ChartType` vocabulary,
  the Observable Plot + JSX-template renderers, the iframe template renderer.
- **Replace the data layer:** rubix-cube fetches over REST (`/api/projects/{id}/…`). In lazybones **every**
  read/write goes through `bridge.call(tool, args)`. `datasets.tsx` (a table over a REST resource) becomes a
  **source browser** over the installed tool surface (`ext.list` + `series.find`), not REST.
- **Don't touch:** the `dashboard:{id}` persistence, the grid host, the trust tiers, the workspace wall.

**Why generalize the binding instead of adding special cases.** The alternative — keep `{series}` binding
and bolt on a `{query}` binding, then a `{bus}` binding, then a `{ext-tool}` binding — multiplies the
contract and the bridge surface per source kind. One `{ tool, args }` source collapses all of them onto the
**one** thing the platform already standardizes on (MCP, rule 7). New source kinds (a future `store.query`,
a new extension's verb) need **zero** dashboard changes — they're just tools.

**Why writes are safe now (the deliberate supersession).** `dashboard-widgets-scope.md` froze "read-only,
four verbs" as a `v:1` contract. This scope **supersedes that to `v:2`** — and it is *less* novel than it
sounds, because the shipped **page** bridge already forwards writes under the grant (`proof-panel`). A
control calling `mqtt.publish` is gated by exactly the same machinery as the page calling `ingest.write`:
the tool must be in `requested ∩ admin_approved`, the host re-checks `mcp:mqtt.publish:call` + the
workspace (from the token, never the cell) on every call. The v1 ban was caution before a concrete control
use-case existed; the capability system already makes a scoped write as safe as a scoped read. **This is a
real, flagged contract revision** — see "The widget contract, v2" — not a quiet change.

**Rejected alternatives:**

- *Keep the four-verb read-only widget contract; put writes in "pages".* Rejected — the user's switch/slider
  and write-capable templates are exactly cell-sized, and forcing each into a full nav page over-builds the
  trust surface for what is one gated tool call. The grant already leashes it.
- *Give widgets a `store.query` super-verb for "direct SurrealDB".* Rejected as a *special case* — if such a
  read-only verb exists it is just another tool the source picker can name; the dashboard needs no special
  binding for it.
- *Run user JSX/Plot/D3 in-process for speed.* Rejected — arbitrary author code in the shell process is RCE.
  Scripted views are iframe-sandboxed; only allow-listed publisher keys federate in-process.
- *Store template code in `localStorage` / the cell blob.* Rejected for durable templates — code is state
  → a `render_templates` SurrealDB row (small inline snippets may live in `cell.options`, bounded).

## The widget taxonomy (every type accounted for)

| `view` | Source result it expects | Renderer (ported from) | Writes? | Trust tier |
|---|---|---|---|---|
| `chart` | rows / a stream | `time-series-chart` (recharts) | no | in-process (built-in) |
| `stat` | a single latest value | built-in | no | in-process |
| `gauge` | a single value + thresholds | built-in | no | in-process |
| `table` | rows | `InfiniteDataTable` (datasets) | no | in-process |
| `plot` | rows | **Observable Plot** snippet | **optional** | iframe (or trusted in-process) |
| `d3` | rows | **D3 / Observable** snippet | **optional** | iframe (or trusted in-process) |
| `template` | rows | **JSX render template** | **optional** | iframe (or trusted in-process) |
| `switch` | (optional) current bool | built-in control | **yes** | in-process |
| `slider` | (optional) current number | built-in control | **yes** | in-process |
| `button` | — | built-in control | **yes** | in-process |
| `ext:<id>/<widget>` | whatever the ext renders | the extension's federated remote | per its grant | trust by publisher key |

- **Read views** call only read tools; their grant subset is read verbs.
- **Scripted views** (`plot`/`d3`/`template`) get a `bridge` inside the iframe and may call **any** tool in
  their cell's declared set ∩ grant — including writes ("render template can write as well — a ton of
  freedom"). The iframe sandbox + the grant + the host re-check are the three guards.
- **Control views** declare an `action = { tool, argsTemplate }`; interacting fills `argsTemplate` (the
  slider value, the switch state) and calls the write tool through the bridge.
- **Extension widgets** are opaque remotes — their reachable tools are their `[[widget]].scope ∩ grant`.

## The source picker — hide MCP from the author

"I don't know from MCP" is a requirement, not an aside: the **author must not think in tool names.** The
builder's left rail is a **source picker** grouped by friendly origin, each entry resolving to a
`{ tool, args }` under the hood:

- **Series** → `series.find` (tag/facet browse) → a chosen series ⇒ `{ tool: "series.read", args }`.
- **Live (Zenoh)** → a subject browser ⇒ `{ tool: "series.watch" | "bus.watch", args }`.
- **An installed extension** (e.g. *my MQTT bridge*) → its read tools from `ext.list`, labelled by the
  manifest (`mqtt.status`, `mqtt.subscribe`) ⇒ `{ tool: "mqtt.status", args }`.
- **Action** (for controls) → the extension's write tools ⇒ the control's `action.tool`.

The picker is literally the "switch / slider / sider for say Zenoh or my MQTT extension" the user asked
for: a control surface that lets a non-MCP-literate user pick a source/action by name. `ext.list` already
returns each install's tools + labels (the data the picker needs is shipped).

## The widget contract, v2 (supersedes the frozen v1 — flagged)

`dashboard-widgets-scope.md` froze a `v:1` widget contract (read-only, four series verbs, no writes). This
scope defines **`v:2`**, additive in shape, broader in reach:

```
mount(el, ctx, bridge)                       // unchanged signature (matches the shipped page mount)
  ctx    = { workspace, binding, options }   // workspace = the hard wall (from the token); + the cell's source+options
  bridge = { call(tool, args) → Promise,     // ANY tool in (cell set ∩ install grant); host re-checks per call
             watch(tool, args, onEvent) → unsubscribe }   // streaming sources (series.watch/bus.watch) over the SSE
```

- **What changed v1 → v2:** the forwardable set is no longer the hardcoded `{series.read|latest|find|watch}`
  — it is **`cell.tools ∩ install-grant`**, which may include write tools. The `[[widget]].scope` may name
  any tool (read or write); the host **rejects an install** whose scope names a tool the admin did not
  approve. Effective reach = `scope ∩ admin_approved`, re-checked per call (the shipped S4 intersection).
- **What did NOT change (still load-bearing):** no token at the widget; workspace from the token, never the
  message; host re-checks cap + workspace + any series/arg narrowing on **every** call; untrusted code only
  in the iframe; `watch` torn down on unmount/uninstall (stateless eviction).
- **Versioning:** every cell, manifest block, and bridge message carries `v`; a receiver rejects an unknown
  major `v`. v2 is what this scope freezes; v1 cells keep working (a v1 cell is a v2 cell whose tool set is
  the four read verbs).

## How it fits the core

- **Tenancy / isolation (rule 6):** the cell, the dashboard, and the `render_templates` row are
  workspace-namespaced; `bridge.call` derives the workspace from the session token, never the cell or the
  iframe message. A ws-B widget (built-in, scripted, or extension) can read/write only ws-B. **Mandatory
  two-session test**, extended to a *write* widget (ws-B's switch cannot publish into ws-A's MQTT topic map).
- **Capabilities (rule 5/7):** the dashboard verbs (`dashboard.save`/`get`/`list`/`delete`/`share`) are
  unchanged. A widget's data/actions reach the store **only** through `bridge.call`, gated by `cell.tools ∩
  grant`, re-checked at the host. New cap surface: **none for reads** (reuse existing); **writes reuse the
  target tool's existing cap** (`mcp:mqtt.publish:call`, `mcp:ingest.write:call`, …) — a widget invents no
  new capability, it just *calls* an already-gated one. The deny path (a widget calling a tool outside its
  grant → denied server-side even if the bridge filter were bypassed) is the headline test, now across
  **write** verbs too.
- **Placement (rule 1):** one builder, two transports (Tauri `invoke` / gateway SSE+HTTP). The control's
  write tool routes through the existing queryable path (`either` placement). No role branch.
- **MCP surface (§6.1) — the API shape:**
  - **Consumed, not exposed:** a widget *consumes* tools (read sources, write actions). The builder needs no
    new write verb of its own beyond the existing `dashboard.save` (the cell, incl. its `source`/`view`/
    `options`/`action`, is part of the layout record — one UPSERT, synchronous, bounded).
  - **Get/list:** the source picker reads `ext.list` (installed tools + labels) and `series.find` — both
    shipped. A `render_templates` CRUD (`template.save`/`get`/`list`/`delete`) is **new** (small, bounded,
    workspace+author-scoped, gated `mcp:template.*:call`) for in-app scripted widgets that persist.
  - **Live feed:** `bridge.watch` is satisfied by the **shipped** series SSE (`GET /series/{s}/stream`) — no
    new transport; a `bus.watch` source maps onto the same SSE mechanism over its subject. No polling.
  - **Batch:** N/A — a user authors one widget at a time. A scripted template that fans a tool over a huge
    range MUST itself call a tool that is a **job** (§6.10) if unbounded; the dashboard does not run
    unbounded loops in a render. Stated, with the bound.
- **Data (SurrealDB):** cells in `dashboard:{id}.cells[]` (extended with `view`/`source`/`action`); inline
  template code optionally in `cell.options` (bounded size); durable scripted templates in a
  `render_templates` table (`render_template:{id}`, workspace-scoped, author-owned). Extension `[[widget]]`
  declarations ride the existing `Install.widgets` (already shipped per `widgets-scope.md`). No new store.
- **Bus (Zenoh):** read streams subscribe to the existing series/bus motion subjects via SSE (best-effort,
  fire-and-forget). A **control's write** with a must-deliver effect (e.g. an actuator command that must
  reach a device) goes through the **outbox**, not raw pub/sub — the control calls a tool whose handler
  enqueues to the outbox (durability is the *tool's* concern, not the widget's). Stated.
- **Sync / authority:** the cell + the `render_templates` row are `(table,id)` upserts on the shipped §6.8
  sync path; a dashboard/template authored on the hub syncs to a workstation idempotently.
- **Secrets:** none reach the widget. A write tool that needs a secret (MQTT password) pulls it server-side
  via `lb-secrets` inside the tool handler; the secret never touches the cell, the iframe, or the bridge.
- **SDK/WIT impact — FLAG LOUDLY.** The **widget contract goes v1 → v2** (forwardable set = `cell.tools ∩
  grant`, writes allowed) and the **`[[widget]].scope`** may now name write tools. These are long-lived
  boundaries; they are specified as **frozen v2** here (the stop-and-confirm gate is discharged in this
  scope), additive over v1, with a `v` field so a future v3 is additive. This is the one boundary change a
  reviewer must sign off.

## Example flow

1. **Install.** An admin installs an `mqtt-bridge` extension into `kfc`, approving
   `mcp:mqtt.status:call`, `mcp:mqtt.publish:call`, `net:tls:broker.acme:8883`. The extension also ships a
   `[[widget]]` tile (`label = "Cooler Switch"`, `scope = ["mqtt.status","mqtt.publish"]`).
2. **Author a read widget (no code).** Alice opens a dashboard in edit mode. The **source picker** shows
   *Series → cooler.temp*. She picks it, the builder runs `series.read`, introspects columns, she chooses
   **chart**, maps `x=ts, y=value`, previews live (`series.watch` over SSE), clicks **Add**. The cell
   `{ view:"chart", source:{tool:"series.read",args:{series:"cooler.temp"}} }` is persisted via
   `dashboard.save`.
3. **Author a scripted widget that writes.** Bob picks **template**, types a JSX snippet that renders the
   latest cooler reading and a "Defrost" button whose `onClick` calls
   `bridge.call("mqtt.publish", {topic:"acme/cooler/defrost", payload:true})`. It saves as a
   `render_template:{id}` row + a cell. It renders in a **sandboxed iframe**; the bridge confirms
   `mqtt.publish ∈ cell.tools ∩ grant`, forwards it; the host re-checks `mcp:mqtt.publish:call` + the `kfc`
   workspace (from Alice's token, not the iframe) → the command is published. Token-less, ws-scoped, gated.
4. **Drop a control (no code).** Carol drags a **switch**, source-picks *Action → mqtt-bridge →
   mqtt.publish*, binds its read to `mqtt.status`. Toggling it calls the write tool through the same bridge.
5. **Use an extension widget.** The `mqtt-bridge` `[[widget]]` "Cooler Switch" appears in the palette
   (Gate 1 install + Gate 2 edit cap). Dragging it creates `ext:mqtt-bridge/cooler-switch`; the publisher
   key is allow-listed → it module-federates in-process; otherwise it iframes. It reaches only
   `mqtt.status`/`mqtt.publish ∩ grant`.
6. **Deny.** Bob's template posts `{tool:"dashboard.delete"}` — not in `cell.tools`. The bridge rejects it;
   the host would deny it regardless (no `mcp:dashboard.delete:call` in the widget's grant). Nothing happens.
7. **Isolation.** Dave (a `mcdonalds` session) builds the same switch; every bridged call is `mcdonalds`; a
   `kfc` topic/series is denied/empty. The wall holds across the **write** bridge.
8. **Uninstall.** The admin uninstalls `mqtt-bridge` → its `ext:` cells render "extension not installed",
   their `watch` streams tear down, the dashboard record still lists the cells. Stateless eviction.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`) — real gateway, real installed reference
extension(s), real tools, seeded real rows; **no `*.fake.ts`**:

- **Capability deny — per verb, now including writes.** A widget calling a tool **outside** `cell.tools ∩
  grant` is denied **server-side** (assert the host denies even if the bridge filter were bypassed): a
  *read* outside scope, a *write* outside scope (`mqtt.publish` with no grant → denied), and a control whose
  action tool the admin did not approve. Deny is opaque.
- **Workspace isolation — including a write widget.** Two real sessions: a ws-B switch cannot publish into
  ws-A's topic map; a ws-B scripted template cannot read/write ws-A series; `ext.list`/source-picker is
  workspace-partitioned. The two-principal test extended to the write bridge.
- **Token never crosses the boundary.** Assert the session token appears in **no** in-process `bridge` arg
  or iframe `postMessage` payload (request, reply, or watch event), for read and write calls alike.
- **Trust-tier routing.** An allow-listed-key extension widget renders in-process; a non-allow-listed one
  (and every scripted `plot`/`d3`/`template`) renders **sandboxed**; a non-allow-listed key cannot federate
  in-process even if its manifest asks.
- **Offline / sync.** The cell + `render_template:{id}` upserts replay idempotently on the §6.8 path.
- **Hot-reload / eviction.** Uninstalling an extension evicts its `ext:` cells and tears down their
  `watch` streams with nothing to clean up; a re-installed extension's widget mounts again.

Plus this slice's cases:

- **Builder round-trip (frontend, real gateway):** source-pick a seeded series → run the source tool →
  columns introspected → choose each view (`chart`/`stat`/`gauge`/`table`/`plot`/`d3`/`template`) → preview
  renders real rows → **Add** persists a cell → reload re-renders it.
- **Write control e2e:** a `switch`/`button` bound to a real ext write tool actually invokes it (assert the
  side effect, e.g. a published sample appears on the series SSE), gated + ws-scoped.
- **Scripted-template write e2e:** an inline JSX/Plot template calls a granted write tool from inside the
  iframe; assert the host effect + the deny when the tool is ungranted.
- **`render_templates` CRUD:** `template.save`→`get`→`list`→`delete`, workspace+author-scoped, deny-per-verb.
- **Extension widget e2e:** install an ext with a `[[widget]]` → palette → drag → `ext:<id>/<widget>` cell →
  bridged calls within `scope ∩ grant` → uninstall evicts cleanly. (Model: `proof-panel`.)
- **Source-picker:** `ext.list` + `series.find` populate the picker by friendly label; the chosen entry
  produces the correct `{tool,args}` cell without the author seeing a tool name.

## Risks & hard problems

- **Write widgets widen the trust surface — the grant must be the real leash.** The entire safety story is
  `cell.tools ∩ install-grant`, re-checked at the host per call (the bridge filter is convenience). If the
  host doesn't re-check a *write* exactly as it re-checks a read, a widget becomes a privileged actor. The
  deny test must bite a **real** ungranted write, not a UI message. **Load-bearing.**
- **Arbitrary author code (Plot/D3/JSX) is RCE if it ever runs in-process.** Scripted views must iframe;
  only allow-listed publisher keys federate. The `sandbox` flags, `event.origin` checks, and CSP must be
  correct or an "untrusted" template isn't sandboxed. Pin and test them.
- **The v1→v2 contract change is a forever-cost.** Get the v2 widget bridge + `[[widget]].scope`
  (write-capable) right once; a v3 is expensive. The stop-and-confirm gate is discharged here — review it.
- **Control durability.** A control whose write must reach a device is a **must-deliver** effect → the
  tool it calls must route through the **outbox**, not raw pub/sub. The widget must not pretend a
  fire-and-forget publish is an actuation ack. State it; the durability is the tool's, not the cell's.
- **Result-shape ↔ view mismatch.** A tool returning a scalar can't be a table; rows can't be a gauge. The
  builder must introspect the result (rubix-cube's `transformDataToColumns`) and offer only valid views,
  degrading honestly (no fake value) when a source returns nothing/denied.
- **Template/source unbounded work.** A scripted view that loops a tool over a huge range blocks the render.
  Bound it; unbounded work must call a **job**-backed tool, not loop in `mount`.
- **Cell/record growth.** Inline template code in `cell.options` bloats the dashboard record; bound inline
  size and push durable templates to `render_templates` (roster stays metadata-only).

## Follow-up slices (post-ship, additive — 2026-06-28)

**Status: ALL THREE SHIPPED (2026-06-28).** See
[widget-builder-followups-session.md](../../../sessions/frontend/widget-builder-followups-session.md);
promoted to [public/frontend/dashboard.md](../../../public/frontend/dashboard.md). Additive over the
shipped v2 surface — no contract change. Backend `store_query_test` 6/6, frontend `toSurrealQL` 8/8
unit + `sqlSource.gateway` 8/8 real-gateway, all green.

- **Slice A — `store.query` + `store.schema`** — SHIPPED: `rust/crates/host/src/store_query/`
  (parse-allowlist by statement kind, workspace-walled, 10k/5s bound) + gateway `POST /store/query` /
  `GET /store/schema` + `ui/.../sql.api.ts` + a "Direct SurrealDB" source-picker entry.
- **Slice B — the CodeMirror editor** — SHIPPED: `ui/.../builder/editors/` (CodeEditor, PlotCodeField,
  TemplateSourceField reading `template.list` over the bridge, SqlEditor) on `@uiw/react-codemirror`;
  `WidgetBuilder` uses them instead of a raw textarea.
- **Slice C — the Grafana-style Builder⇄Code SQL editor** — SHIPPED: `ui/.../builder/sql/`
  (`SqlBuilderQuery` + `toSurrealQL` + SqlQueryEditor/Header/VisualEditor/RawEditor); the cell stores
  both the raw string and the builder query; Builder only generates SELECT, Code stays
  parse-allowlisted by `store.query`.

The shipped truth is below for reference; the original ask text is unchanged.

### Slice A — `store.query`: a read-only SQL-to-SurrealDB host tool (the "direct SurrealDB" source)

The scope always said "direct SurrealDB = a read-only `store.query`-style tool **if one exists**." Build it.
It is a normal host MCP verb (like `series.read`/`ingest.write`), so the dashboard needs **zero** new
binding — the source picker gains a **"SQL query"** entry that produces `{ tool: "store.query", args: { sql,
vars? } }`, and every existing view (table/chart/stat/plot/template) renders its rows unchanged.

- **Host service:** `crates/host/src/store_query/{mod,authorize,run,tool}.rs` (one verb per file,
  FILE-LAYOUT). `store.query(sql, vars?) -> { columns, rows }`, gated **`mcp:store.query:call`**.
- **READ-ONLY, enforced — load-bearing.** The handler MUST reject anything but a single read. Enforce by
  **parsing the statement** (SurrealDB's parser / `surrealdb::sql::parse`) and allowing **only** `SELECT`
  (and `INFO`/`SHOW`-class introspection if needed) — reject `CREATE/UPDATE/DELETE/DEFINE/REMOVE/RELATE/
  INSERT/UPSERT`, multiple statements, and any transaction control. A string `LIKE '%delete%'` check is NOT
  acceptable — parse and allowlist the statement kind. Mutation goes through real typed write tools, never
  this verb.
- **Workspace wall (rule 6):** the query runs **inside the caller's workspace namespace**, set host-side
  from the session token — never a `USE NS/DB` or a workspace named in the SQL. A ws-B caller's
  `store.query` can reach only ws-B records, structurally (the same namespacing every other store read
  uses). The SQL cannot escape the namespace; reject any statement that names one.
- **Bounded (rule §6.1 / §6.10):** a hard **row cap** and a **statement timeout** (config, e.g. 10k rows /
  5 s); the handler injects/enforces a `LIMIT` ceiling. An unbounded analytical scan MUST be a **job**, not
  this synchronous verb — `store.query` is for interactive, bounded reads. Stated, with the bound.
- **It's just a tool for the widget bridge.** A scripted template/control may call `store.query` only if
  `mcp:store.query:call ∈ cell.tools ∩ install-grant`, re-checked at the host per call — same leash as every
  other tool. No special widget path.
- **The SQL editor** (below) gets a `@codemirror/lang-sql` mode; the SurrealQL dialect is close enough to
  SQL for highlighting (a SurrealQL grammar refinement is a named follow-up, not a blocker).
- **Tests (mandatory):** deny without `mcp:store.query:call`; **a write statement is rejected** (parse-level,
  per kind — `CREATE/UPDATE/DELETE/DEFINE/RELATE/INSERT` each denied); **two-session isolation** (ws-B SQL
  cannot read ws-A rows; a workspace-naming statement is rejected); row-cap + timeout enforced; a `SELECT`
  round-trips real seeded rows into a `table`/`chart` widget end to end.

### Slice B — the in-app code editor (port rubix-cube's CodeMirror editor for scripted views)

The shipped `plot`/`d3`/`template` views render code, but the builder needs the **authoring editor**. Port
it from rubix-cube — it is **CodeMirror**, not Monaco (lighter, already the rubix-cube choice):

- **Library:** `@uiw/react-codemirror` + `@codemirror/lang-javascript` (`javascript({ jsx: true })`) for the
  JSX/Plot/D3 editor, `@codemirror/lang-sql` for the `store.query` SQL editor, `EditorView.lineWrapping`, a
  shared theme. Add these deps to `ui/package.json` (mirror rubix-cube versions: `@uiw/react-codemirror`
  `^4.25.x`, the `@codemirror/*` `^6.x` set).
- **Port these components** (data layer swapped to `bridge.call`, REST/SWR/`next` removed):
  - `components/ui/template-renderer/manage-template-dialog/code-editor.tsx` → the JSX template editor
    (`Controller`-free; wire to the builder store, not react-hook-form unless already present).
  - `components/dashboards/editor/fields/PlotCodeField.tsx` → the **Plot** editor (snippet convention
    `({ data, Plot, d3 }) => element`, bindings hint shown; `DEFAULT_PLOT_CODE` carried over).
  - `components/dashboards/editor/fields/TemplateSourceField.tsx` → the **template** source field
    (inline-code tab **or** a saved `render_templates` pick; `DEFAULT_INLINE_CODE = ({ data }) => <jsx>`),
    its "saved template" list reading the new `template.list` verb instead of REST.
  - `components/sql/sql-editor.tsx` → the **raw SQL** CodeMirror editor used as the **Code** half of the
    Grafana-style Builder⇄Code SQL source (Slice C). Drop the `/api/.../sql/generate` AI button (re-pointing
    it at an MCP `sql.generate` tool is a named follow-up, out of scope here).
- **Where it lives:** `ui/src/features/dashboard/builder/editors/{CodeEditor,PlotCodeField,TemplateSource
  Field,SqlEditor}.tsx` — one component per file (FILE-LAYOUT). The editor only edits the snippet string
  that goes into `cell.options.code` (≤4 KB inline) or a `render_template:{id}` row (≤64 KB); it holds no
  data and no token (the iframe runtime, not the editor, calls the bridge).
- **Trust unchanged:** the editor authors code; that code still executes **only** in the sandboxed iframe
  (or trusted-key in-process). Editing is in the trusted shell; running is sandboxed. No change to the v2
  contract.
- **Tests:** the editor round-trips a snippet into `cell.options.code` / a `render_template` row and the
  saved snippet renders in the iframe (reuse the shipped scripted-template e2e); the SQL editor's text
  drives a `store.query` source that renders a `table` widget (Slice A integration).

### Slice C — the SQL source as a Grafana-style **Builder ⇄ Code** editor (not a bare textarea)

Slice B alone gives the *raw SQL* (Code) half. The SQL source must match **Grafana's `grafana-sql` model**:
a **visual query Builder** with a toggle to **Code** (raw SQL), the two kept in sync. A non-SQL user builds
`Table → Column/Aggregation → Filter → Group by → Order/Limit → Preview`; a power user flips to Code and
edits raw SurrealQL. This is the missing "query builder" — what we ported from rubix-cube was a *chart*
builder (map x/y over already-fetched rows), **not** a *query* builder.

**Port from Grafana `grafana-sql`** (pinned `940590ff56f730534c715299f2d4386a42e24368`) — copy the
structure, strip `@grafana/*` runtime/`@grafana/ui` deps, render with our shadcn primitives, and emit a
SurrealQL string + run via `store.query`:

| Grafana file | Port to `ui/src/features/dashboard/builder/sql/` | Role |
|---|---|---|
| `packages/grafana-sql/src/components/QueryEditor.tsx` | `SqlQueryEditor.tsx` | switches Builder vs Code by `editorMode` |
| `packages/grafana-sql/src/components/QueryHeader.tsx` | `SqlQueryHeader.tsx` | the **Builder / Code** toggle + confirm-on-switch-back (don't silently clobber hand-edited raw SQL) |
| `packages/grafana-sql/src/components/visual-query-builder/VisualEditor.tsx` | `VisualEditor.tsx` | the rows: select, filter (where), group by, order by, limit, preview |
| `packages/grafana-sql/src/components/query-editor-raw/RawEditor.tsx` | `RawEditor.tsx` | wraps the raw CodeMirror SQL editor (Slice B's `SqlEditor.tsx`) |
| `packages/grafana-sql/src/components/query-editor-raw/QueryEditorRaw.tsx` | folded into `RawEditor.tsx` | writes the raw string back into the source args |

(The **Loki** builder/raw files are the same pattern for a non-SQL query language — keep them as the
reference for a *future* LogQL-style source; do **not** port them now. Named follow-up.)

- **The query model → a SurrealQL string.** The visual builder edits a typed `SqlBuilderQuery`
  (`{ table, columns:[{name, aggregation?}], filters:[…], groupBy:[…], orderBy?, limit? }`); a
  `toSurrealQL(query)` renderer (one file, the analog of Grafana's SQL `expressionBuilder`) emits the
  `SELECT … FROM … WHERE … GROUP BY … ORDER BY … LIMIT …` string. The cell stores **both** the raw string
  (what `store.query` runs) and, when in Builder mode, the `SqlBuilderQuery` (so reopening returns to the
  builder). Switching Builder→Code regenerates the string; Code→Builder asks to confirm (Grafana's
  behavior) because hand-edited SQL may not round-trip.
- **The builder needs a schema source — extend Slice A.** The Table/Column dropdowns need the workspace's
  tables and their columns. Add a tiny read-only host verb **`store.schema() -> { tables:[{name,
  columns:[{name,type}]}] }`** (gated `mcp:store.schema:call`, workspace-walled from the token, derived
  from SurrealDB `INFO FOR DB`/`INFO FOR TABLE`). The source picker's "SQL query" entry calls `store.schema`
  to populate the builder; `store.query` runs the result. Both are just tools on the bridge (leashed by the
  grant), no special path.
- **Format toggle (Grafana's "Format: Table"):** keep a small `format` on the SQL source (`table` |
  `time-series`) that shapes the result for the chosen view — `table` passes rows through; `time-series`
  asserts a time column. Map it to the existing view selection rather than a parallel concept.
- **Trust/bounds unchanged:** Builder mode can only generate `SELECT` (it has no syntax for a write), and
  Code mode is still parsed + allowlisted to `SELECT` by `store.query` (Slice A). The row-cap/timeout and
  the workspace wall apply to whatever string runs, builder- or hand-authored. The builder is convenience;
  `store.query`'s parse gate is the boundary.
- **Tests (mandatory):** `store.schema` deny + isolation (ws-B sees only ws-B tables); a built query renders
  to the expected SurrealQL string (`toSurrealQL` unit cases: columns, aggregation, filter, group-by,
  order, limit); Builder→Code→Builder round-trips a builder-authored query; a Code-mode write is still
  rejected by `store.query`; an end-to-end "build a query in the visual editor → Run → rows render in a
  `table`/`chart` widget" on real seeded tables.

Both/all slices write a session doc, promote shipped truth to `public/frontend/dashboard.md`, and keep the
mandatory deny + isolation tests green. They do **not** touch the frozen v2 widget/bridge contract.

## Open questions

Decisions made so the build has no blocking open question; residuals are named follow-ups, not gaps.

**Resolved (decisions taken):**

- **Source = `{ tool, args }`, any granted tool, read or write.** Decided — supersedes the `{series}`-only
  binding (additive; a v1 series cell is a v2 cell).
- **Widget contract → v2** (forwardable set = `cell.tools ∩ grant`; writes allowed; host re-checks per
  call). Decided and frozen here; flagged as the SDK/WIT boundary change.
- **Scripted views (Plot/D3/JSX) iframe-only** unless the publisher key is allow-listed. Decided.
- **View set v1:** `chart/stat/gauge/table/plot/d3/template/switch/slider/button` + `ext:<id>/<widget>`.
  Decided; more views are additive.
- **Author never types MCP** — the source picker maps friendly labels (via `ext.list`/`series.find`) to
  tools. Decided.

**Named follow-ups — RESOLVED in build (2026-06-27, the `lean` taken in each case). See
[widget-builder-session.md](../../../sessions/frontend/widget-builder-session.md).**

1. **The `ext:<id>/<widget>` cell key** — RESOLVED `ext:<extension-id>/<widget-id>`
   (`ui/src/features/dashboard/builder/ExtWidget.tsx` `parseExtKey`); the key names the tile.
2. **Widget expose for extensions** — RESOLVED a named **`mountWidget`** export on the SAME
   `remoteEntry.js` as the page (one build). Proven by `proof-panel` (`ui/src/mount.tsx` +
   `remoteEntry.ts`).
3. **`render_templates` vs inline** threshold — RESOLVED a small inline cap **`INLINE_MAX_BYTES = 4 KB`**
   in `cell.options.code`; larger ⇒ a `render_template:{id}` row (hard cap `TEMPLATE_MAX_BYTES = 64 KB`).
4. **Argument templating for controls** — RESOLVED a typed **`args_template`/`argsTemplate`** with one
   `{{value}}` slot, type-preserving (`views/argsTemplate.ts`).
5. **Does a control read its own state?** — RESOLVED **yes, optional**: a control view carries an optional
   `source` it reads (`SwitchControl` reflects it into the toggle).

## Related

- [`../../dashboard-scope.md`](../../dashboard-scope.md) — Phase 1 grid + the `dashboard:{id}` cell record
  this extends (`view`/`source`/`action` fields).
- [`../../dashboard-widgets-scope.md`](../../dashboard-widgets-scope.md) — the **v1** widget contract this
  **supersedes to v2** (read-only → grant-leashed read/write); the trust tiers + bridge this inherits.
- [`widgets-scope.md`](widgets-scope.md) — the shipped built-ins + the `[[widget]]` declaration; its
  "render `ext:<id>` in a cell" follow-up is built here.
- [`../../extensions/reference-extensions-scope.md`](../../extensions/reference-extensions-scope.md) — the
  `mqtt-bridge`/`zenoh-gateway`/`timescale` extensions whose tools are the write sources/controls here.
- [`../../extensions/ui-federation-scope.md`](../../extensions/ui-federation-scope.md) — the page bridge
  (`bridge.call`, `mount(el, ctx, bridge)`) this narrows to a cell; `proof-panel` is the working model.
- `rust/extensions/proof-panel/` — the **reference** extension (federated page + `[ui] scope` that already
  forwards writes); the model for an extension-shipped widget.
- rubix-cube `frontend/components/chart-builder/`, `charts/time-series-chart/`, `datasets/` — the view +
  builder layer ported here (data layer swapped REST → `bridge.call`).
- [Observable Plot](https://observablehq.com/plot/) · [Observable / D3](https://observablehq.com/) — the
  `plot`/`d3` scripted-view renderers.
- README **§6.1** (API shape), **§6.13** (extension UIs — federation vs iframe by trust), **§6.6** (3
  gates), **§7** (tenancy), **§3** (rules 4/5/6/7 — stateless, capability-first, the wall, MCP-as-contract).
