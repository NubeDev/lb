# Frontend dashboard scope — widget config + a Grafana-style variable system (vars, refresh, live, JSON payloads)

Status: scope (the ask). Promotes to [`public/frontend/dashboard.md`](../../../public/frontend/dashboard.md)
once shipped. Builds on the **shipped** widget-builder v2 ([`widget-builder-scope.md`](widget-builder-scope.md))
and the widget-palette slice ([`widget-palette-scope.md`](widget-palette-scope.md)).

Editing a widget today is "delete it and add a new one" — there is no per-cell settings surface, no
title, no way to reconfigure a cell after it lands. And every cell is a *fixed* `{tool, args}`: there is
no shared **context** a dashboard passes to its widgets (a selected host, an env, a time range), so the
same panel can't be re-pointed without re-authoring. This scope designs **(1)** a proper widget
settings/config surface and **(2)** a **Grafana-style variable system** — `$var` / `${var}` / `[[var]]`
template variables with built-in globals (`$__from`, `$__to`, `$__interval`, `${__user.login}`, …), URL
sync (`?var-host=web01`), an auto-refresh picker, live push events, and a **JSON payload builder** — all
riding **one shared interpolation library** that extensions reuse, over **one variable model that resolves
from SurrealDB, SSE, Zenoh, an extension tool, or static JSON**.

> Reference doctrine (Grafana): [dashboard variables overview](https://grafana.com/docs/grafana/latest/visualizations/dashboards/variables/),
> [add/configure variables](https://grafana.com/docs/grafana/latest/visualizations/dashboards/variables/add-template-variables/),
> [URL variables](https://grafana.com/docs/grafana/latest/visualizations/dashboards/build-dashboards/create-dashboard-url-variables/),
> [Prometheus template variables](https://grafana.com/docs/grafana/latest/datasources/prometheus/template-variables/). Live/refresh patterns ported from
> rubix-cube (`frontend/lib/events/use-workspace-bus-stream.ts`, `frontend/lib/time.ts`,
> `frontend/components/ui/date-range-filter/store.tsx`).

---

## The headline idea: one variable = a name bound to a *resolver*, interpolated everywhere through one library

A Grafana variable is "a value (or list) you pick once and reference by name across the dashboard." We adopt
that verbatim, and we make the **resolver** the same `{ tool, args }` the widget builder already uses — so a
variable's options can come from **any granted source** with zero special cases:

```
Variable = {
  name, label, type,
  // how its option list / value is produced:
  query?:  { tool, args },     // a granted MCP tool — SurrealDB (store.query), series (series.find),
                               //   an extension verb (timescale.tags), a Zenoh read (bus.watch snapshot)
  custom?: string[],           // a static list
  text?:   string,             // a free textbox default
  const?:  string,             // a hidden fixed value
  interval?: string[],         // a duration list ($__interval style)
  multi, includeAll, current   // selection state (current lives in the URL, not the record)
}
```

| Grafana variable type | Lazybones mapping (no special case) |
|---|---|
| **Query** | `query: { tool, args }` — rows → options. `store.query` (SurrealDB), `series.find` (series), `<ext>.<verb>` (extension), `bus.watch` snapshot (Zenoh) all fit. |
| **Custom** | `custom: ["prod","staging"]` |
| **Text box** | `text: "<default>"` |
| **Constant** | `const: "<value>"` (hidden) |
| **Interval** | `interval: ["1m","5m","1h"]` → feeds `$__interval` |
| **Data source** | `source` — a query over `ext.list`/`series.find` (pick an installed source by label); thin variant of Query. |
| **Ad hoc filters** | **follow-up** — needs `store.schema` (shipped) to offer key/op/value; deferred, named below. |
| **Global / built-in** | resolved shell-side from trusted state (below), never a record/cell value. |

The same value then **interpolates into anything**: a cell's `source.args`, a control's `action.argsTemplate`,
a `store.query` SQL (`vars`-bound, not string-spliced), a **JSON payload** sent to an extension or the bus.
This is the "kick ass vars system for SurrealDB, SSE, Zenoh, extensions, JSON" — one model, one library, five
sinks.

## Goals

- **Widget settings/config.** Edit a cell after it's added: **name/title**, its source/view/options, and which
  variables it consumes — a settings drawer reusing the builder's fields in an "edit existing cell" mode.
  Persisted via the existing `dashboard.save` (no new verb).
- **Dashboard variables (Grafana parity).** Define variables (the types above) on the dashboard; a **variable
  bar** at the top with dropdowns (single/multi/include-all); **selected values live in the URL**
  (`?var-name=value`, shareable), definitions live on the record.
- **The built-in globals.** `$__from`, `$__to`, `$__interval`, `$__interval_ms`, `$__range`, `$__range_s`,
  `$__range_ms`, `${__user.login}`, `${__user.email}`, `${__dashboard}`, and the lazybones-specific
  `${__workspace}` — resolved from the **session + the URL time range**, never from the cell (un-spoofable).
- **One shared interpolation library** (`ui/src/lib/vars/`) — pure TS, no React, no shell deps — implementing
  the three Grafana syntaxes (`$var`, `${var}`, `[[var]]`), format hints (`${var:json}`, `${var:csv}`,
  `${var:singlequote}`, `${var:pipe}`), multi-value expansion, and the built-ins. **Extensions import the same
  library**, and the shell also passes **resolved vars into the widget `ctx`** so a packaged widget gets them
  for free.
- **Auto-refresh + live events.** A refresh-interval picker (`off`/`5s`/`10s`/`30s`/`1m`/`5m`/`15m`,
  URL-synced `?refresh=30s`) that re-resolves query variables + re-runs each cell's read source; **plus** live
  push via `series.watch`/`bus.watch` folded in (state-vs-motion: refresh re-reads state, watch streams
  motion).
- **JSON payload builder.** A template editor that authors a JSON body with `${var}` / `{{value}}` slots and a
  **target** (an extension tool like `todo.add`, a `bus.publish(subject, payload)`, or `ingest.write`);
  on send it interpolates and calls the target through the bridge. Generalizes the shipped control
  `argsTemplate` to a full var-aware JSON body.
- **The one new backend surface: generic `bus.publish` / `bus.watch`.** A workspace-walled, capability-gated
  arbitrary-subject pub/sub (the SSE/Zenoh sink + the Zenoh/live source) — see "Platform fix" below.

## Non-goals

- **No new datastore / compute plane.** Variables are records on the dashboard; option lists come from the
  source tool. Aggregation is the tool's job.
- **No string-spliced SQL.** A `store.query` variable/cell binds values as `vars` parameters (the shipped
  `{ sql, vars }`); interpolation into raw SQL text is only for non-injectable spots (and `store.query` still
  parse-allowlists). The JSON builder interpolates into a JSON value tree, not a query string.
- **No un-leashed sink.** A JSON payload can target only a tool in the cell's tool set ∩ install grant
  (bridge leash, re-checked at the host) — `bus.publish` included. Generalizing controls to JSON does not
  widen what a widget may call.
- **No identity vars from the client.** `${__user.*}`/`${__workspace}` are resolved shell-side from the
  verified token; a cell/iframe can never set them. No exception.
- **No ad-hoc filters in v1** (deferred, named follow-up). No alerting/annotations (out of scope).
- **No `*.fake.ts`.** Real gateway, real installed extension, real seeded rows, real bus subject.

## Intent / approach

**Make the variable the unit, the resolver a `{tool,args}`, and interpolation a single pure library; layer
config + refresh + live + JSON on top.** Five slices, build-ordered; each is independently shippable and the
library underpins them all.

### The shared `vars` library (`ui/src/lib/vars/`) — the spine, extensions reuse it

A pure-TS module, **no React, no `@/` shell imports**, so it bundles into both the shell and a federated
extension remote (and is also exposed as a shared singleton on the federation import-map, like React):

- `types.ts` — `Variable`, `VarScope` (`{ values: Record<string, string|string[]>, builtins: Builtins }`),
  `FormatHint`.
- `interpolate.ts` — `interpolate(template: string, scope: VarScope): string` handling `$var`, `${var}`,
  `[[var]]`, `${var:format}` (json/csv/singlequote/doublequote/pipe/raw), multi-value expansion (Grafana's
  `{a,b}` / regex / glob forms — start with csv+pipe+json, the rest named), and **leaving an unknown var
  literal** (Grafana behavior) unless `strict`.
- `interpolateValue.ts` — `interpolateArgs(argsTree: unknown, scope): unknown` — the deep substitution over a
  JSON value tree (generalizes the shipped `views/argsTemplate.ts` `subst`; `{{value}}` becomes the built-in
  `${__value}`). This is what the JSON payload builder and every cell `source.args`/`action.argsTemplate` run
  through before a bridge call.
- `builtins.ts` — `resolveBuiltins({ timeRange, identity, dashboardId, workspace })` → the `$__*` / `${__user.*}`
  / `${__dashboard}` / `${__workspace}` map. **Pure** — given trusted inputs; the shell supplies them from the
  token + URL, never the cell.
- `parse.ts` — `extractVarNames(template)` (which vars a string/args-tree references — used to compute a
  cell's variable dependencies for refresh + the deny-set).

The shell's React layer (`ui/src/features/dashboard/vars/`) owns the **stateful** half: the variable bar,
the value store synced to the URL, and query-variable resolution over the bridge. **Extensions never resolve
identity or query variables themselves** — the shell resolves the scope and hands a resolved
`ctx.vars`/`ctx.timeRange` to the widget; an extension that wants to build its own payload imports
`interpolateArgs` from the shared lib and runs it over the resolved scope it was given.

### Slice 1 — widget settings/config (edit, not re-add)

- **Cell gains `title`** (additive `#[serde(default)]` on `Cell`, FE `Cell.title?`). The header renders it;
  default falls back to a derived label.
- **A settings drawer** (`features/dashboard/builder/CellSettings.tsx`) opened from a per-cell ⚙ button in
  edit mode. It reuses the builder's source/view/option fields in an **edit-existing** mode (seed state from
  the cell, write back on save). Persists the whole dashboard via `dashboard.save` (round-trips today).
- **Gate:** the ⚙/edit affordance is shown only with `mcp:dashboard.save:call` (the widget-palette gate,
  reused); the server re-checks on save.

### Slice 2 — the variable model + bar + URL sync

- **`Dashboard.variables: Vec<Variable>`** (additive `#[serde(default)]`; round-trips today, no new verb).
- **The variable bar** (`features/dashboard/vars/VariableBar.tsx`) renders a dropdown per variable;
  query-type options resolve over the bridge (`useVariableOptions`, a `{tool,args}` read leashed by the
  dashboard's tool set ∩ grant). **Selected values sync to the URL** (`?var-<name>=value`, repeated for
  multi) via the shipped TanStack search params (extend `validateDashboardSearch`); the time range stays
  `?from`/`?to` and feeds `$__from`/`$__to`.
- **Variable editor** (in the dashboard settings) to add/edit/reorder variables (type, name, label, query
  source via the source picker, multi/include-all).

### Slice 3 — interpolation wired into every cell call

- Before any cell `bridge.call(source.tool, source.args)` or control `bridge.call(action.tool, filled)`, run
  `interpolateArgs(args, scope)` with the resolved `VarScope`. For `store.query` cells, interpolate into the
  bound `vars` (and only the safe parts of `sql`), preserving the parse-allowlist.
- Pass `ctx.vars` (resolved values) + `ctx.timeRange` to `ExtWidget`/`mountWidget` (additive v2 ctx field —
  flagged below). A packaged widget reads `ctx.vars.host` directly.

### Slice 4 — auto-refresh + live events

- **Refresh picker** (`features/dashboard/RefreshControl.tsx`, URL `?refresh=30s`): on tick, bump a
  `refreshKey` that re-resolves query variables + re-runs each cell's read source (the proof-panel
  `refreshKey` pattern, generalized). `off` disables.
- **Live push:** generalize the shipped series SSE consumption — `series.watch` already streams; wire
  `bus.watch` (Platform fix) so a cell/variable can subscribe to a Zenoh subject and fold motion in. Refresh
  (poll state) and watch (push motion) compose; a cell declares which it uses.

### Slice 5 — the JSON payload builder

- **`features/dashboard/builder/JsonPayloadField.tsx`** — a CodeMirror JSON editor (reuse the Slice-B editor
  infra) authoring a JSON template with `${var}`/`{{value}}` slots, plus a **target picker** (an extension
  write tool via the source picker's Action group, `bus.publish`, or `ingest.write`).
- On send (a button control, a control's interaction, or a row action), `interpolateArgs(template, scope)` →
  `bridge.call(target, payload)`. The "add todo" example: target `todo.add`, payload
  `{ "text": "${newTodo}", "ws": "${__workspace}" }`. Sending "over SSE/Zenoh" = target `bus.publish` with
  `{ subject, payload }`.

### Platform fix — generic `bus.publish` / `bus.watch` (the one missing API)

Today the bus is reachable only through **series**-scoped verbs (`series.*` + `GET /series/{s}/stream`); there
is **no generic subject pub/sub**. The JSON-over-SSE sink, Zenoh-sourced variables, and live events on
non-series subjects need it. Add it mirroring `ingest`/`series` (one verb per file, FILE-LAYOUT):

- **`crates/host/src/bus/{publish,watch,subscribe,authorize,tool}.rs`** —
  `bus.publish(subject, payload) -> {ok}` (fire-and-forget motion) and `bus.watch(subject) -> stream`.
- **Workspace wall (rule 6):** the subject is namespaced **`ws/{id}/ext/{subject}`** host-side from the token;
  the caller's `subject` is a suffix under the wall — it **cannot name another workspace's subject**
  (structurally, like `series_key`). Reserved prefixes (`series/`, `channels/`, internal motion) are rejected
  so a caller can't impersonate platform motion.
- **Capabilities (rule 5):** gated `mcp:bus.publish:call` / `mcp:bus.watch:call`; deny is opaque. An extension
  requests them in its manifest like any verb; a widget reaches them only via `cell.tools ∩ grant`.
- **State vs motion (rule 3):** `bus.publish` is **fire-and-forget** motion — NOT durable. A must-deliver
  effect still goes through the **outbox** (the JSON builder targeting a must-deliver action calls a tool that
  enqueues; the widget must not pretend a publish is an ack). Stated.
- **Gateway:** `GET /bus/{subject}/stream?token=` (generalize `series_stream.rs`, same `?token=` + `subscribe`
  auth-first 401/403) and `POST /bus/publish` (mirror `/ingest`); both also via `POST /mcp/call`.
- **This is a shared surface, not dashboard-private** — `reference-extensions-scope.md`'s mqtt/zenoh bridges
  and any extension wanting workspace pub/sub use the same verbs. (If preferred, this fix can be extracted to
  `scope/bus/bus-pubsub-scope.md`; specced here because the dashboard is the driving caller.)

**Rejected alternatives:**

- *A bespoke `dashboard.vars`/`widget.config` verb set.* Rejected — variables and titles are fields on the
  dashboard record that `dashboard.save` already round-trips; inventing verbs adds surface for nothing.
- *Per-variable-type resolver code paths (a query resolver, a series resolver, a zenoh resolver…).* Rejected —
  collapses to one `{tool,args}` over the bridge; a new source kind is just a new tool (rule 7).
- *Resolve identity/query vars inside the extension/iframe.* Rejected — `${__user.*}` from untrusted code is a
  spoofing hole; the shell resolves the scope from the token and passes resolved values in `ctx`.
- *String-splice variable values into SQL.* Rejected — injection. Bind as `vars` parameters; `store.query`
  parses + allowlists regardless.
- *Store selected values on the record.* Rejected for *selection* — selection is per-viewer and shareable, so
  it lives in the URL (Grafana parity); only the *definitions* are durable.
- *Reuse the series SSE for arbitrary subjects.* Rejected — series stream is series-keyed; a generic subject
  needs the namespaced `bus.watch` so the wall and the reserved-prefix guard are explicit.

## The variable syntax & built-ins (the contract)

```
$var            ${var}            [[var]]                 # three reference forms (Grafana parity)
${var:json}     ${var:csv}        ${var:singlequote}      # format hints (multi-value aware)
${var:doublequote}  ${var:pipe}   ${var:raw}
```

Built-ins (resolved shell-side; never cell-set):

```
$__from  $__to                    # from/to as epoch ms (the URL time range)
$__from:date  $__to:date          # ISO formatting hints
$__interval  $__interval_ms       # the chosen interval variable / auto
$__range  $__range_s  $__range_ms # to-from span
${__user.login}  ${__user.email}  # the verified session identity
${__dashboard}                    # the dashboard id/title
${__workspace}                    # the tenant (lazybones-specific; replaces Grafana ${__org})
${__value}                        # a control's current value (generalizes the shipped {{value}})
```

Unknown variables interpolate to themselves (Grafana behavior), surfaced as a soft authoring warning, never a
throw — a shared link always renders.

## How it fits the core

- **Tenancy / isolation (rule 6):** variable definitions + cell titles are on the workspace-scoped dashboard
  record; query-variable resolution and the JSON sink derive the workspace from the session token, never the
  cell/URL. `${__workspace}`/`${__user.*}` are the *viewer's*, from the token. `bus.*` subjects are
  `ws/{id}`-walled. **Mandatory two-session test:** ws-B's variable query/`bus.watch`/`bus.publish` reaches
  only ws-B; a ws-B viewer of a shared dashboard sees ws-B data and *their own* `${__user}`.
- **Capabilities (rule 5/7):** edit affordance gated `mcp:dashboard.save:call`; a variable's query and the
  JSON sink are gated by the **target tool's existing cap** ∩ the dashboard's tool set, re-checked at the
  host per call. New caps: only `mcp:bus.publish:call` / `mcp:bus.watch:call` for the new verbs. The deny path
  (a variable/payload calling an ungranted tool, a `bus.*` subject outside the grant) is the headline test.
- **Placement (rule 1):** one shell, two transports; the refresh timer + variable bar are client-side; `bus.*`
  routes through the existing queryable path. No role branch.
- **MCP surface (§6.1):**
  - **Consumed:** variable options + cell reads are existing read tools (`store.query`/`series.*`/`<ext>`);
    the JSON sink is an existing write tool or the new `bus.publish`.
  - **Get/list:** the source picker (`ext.list`/`series.find`/`store.schema`) — shipped.
  - **Live feed:** the shipped series SSE + the **new `bus.watch`** SSE; no polling for motion (refresh polls
    *state* on purpose).
  - **New verbs:** `bus.publish` (fire-and-forget, bounded payload) + `bus.watch` (stream). No batch; a
    variable query that could scan huge ranges must call a **job**-backed tool (stated bound).
  - **No new dashboard verb** — `dashboard.save`/`get` round-trip `variables` + `title` (additive serde).
- **Data (SurrealDB):** `Dashboard.variables[]` + `Cell.title` (additive). Selected values are **not** stored
  (URL). No new table.
- **Bus (Zenoh):** `bus.publish` is motion (fire-and-forget); must-deliver effects go through the **outbox**.
  `bus.watch` subscribes workspace-walled subjects.
- **Sync / authority:** the dashboard record (with variables/titles) is a §6.8 idempotent upsert; it syncs
  edge↔hub like any dashboard.
- **Secrets:** none reach the widget/variable; a sink tool needing a secret pulls it server-side.
- **SDK/WIT impact — FLAG.** The **v2 widget `ctx` gains `vars` + `timeRange`** (additive, versioned — a v2
  widget that ignores them is unaffected). The **shared `vars` library** becomes a stable contract extensions
  link against (and a federation-shared singleton). Both are additive boundaries to freeze with a `v` field;
  the bridge `call`/`watch` signature is unchanged. `bus.*` is a capability-grammar + verb addition, not a
  WIT change.

## Example flow

1. **Define a variable.** On dashboard `Ops`, Alice adds a **Query** variable `host`: source
   `store.query`, `SELECT name FROM host`. The bar shows a `host` dropdown (multi + "All"). She adds an
   **Interval** variable `step = [1m,5m,1h]`.
2. **Reference it.** A chart cell's source is `series.read` with `args.series = "cpu.${host}"`,
   `args.step = "$__interval"`, range `$__from`/`$__to`. A `store.query` table cell binds
   `WHERE host = $host AND ts > $__from` with `vars: { host: "${host}", __from: "$__from" }`.
3. **Pick + share.** Alice selects `host=web01`, range last-6h, refresh 30s. The URL becomes
   `#/t/acme/dashboards?var-host=web01&from=now-6h&to=now&refresh=30s`. She copies it; Bob opens it and sees
   the same view scoped to `web01` — but `${__user.login}` renders **Bob's** login (token-resolved).
4. **Auto-refresh + live.** Every 30s the query variables + cell sources re-run; a cell also `bus.watch`es
   `cooler/alerts` and folds pushed JSON in live.
5. **Edit a widget.** Bob clicks ⚙ on the chart, renames it "Web01 CPU", switches the view to `stat`, saves —
   `dashboard.save` persists the cell's new `title`/`view`. No re-add.
6. **JSON payload (add todo).** A **button** control's JSON builder authors `{ "text": "${newTodo}", "ws":
   "${__workspace}" }` targeting `todo.add`; `newTodo` is a textbox variable. Click → interpolate → 
   `bridge.call("todo.add", {...})`, gated + ws-scoped. A second button targets `bus.publish` with
   `{ subject: "ui/banner", payload: { msg: "${newTodo}" } }` — broadcast over the new SSE.
7. **Deny.** A payload targeting `dashboard.delete` (not in the cell's tool set) is rejected by the bridge and
   would be host-denied regardless. A `bus.publish` to `series/...` (reserved) or another ws is refused.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`) — real gateway, real installed extension(s), real
seeded rows, a real bus subject; **no `*.fake.ts`**:

- **Capability deny (headline):** a variable query / JSON payload / control calling a tool **outside**
  `cell.tools ∩ grant` is denied **server-side** (read and write); `bus.publish`/`bus.watch` **without** the
  cap → denied; a `bus.*` subject naming another workspace or a reserved prefix → refused. Deny opaque.
- **Workspace isolation:** two real sessions — ws-B's variable options, `bus.watch`/`bus.publish`, and cell
  reads reach only ws-B; a shared dashboard renders the **viewer's** `${__user}`/`${__workspace}`; the URL
  var values can't cross the wall.
- **Identity un-spoofable:** an iframe/extension cannot set `${__user.*}`/`${__workspace}`; assert the shell
  resolves them from the token and the cell/postMessage value is ignored. The token appears in no bridge arg
  or iframe payload (read, write, watch).
- **Interpolation (unit, the shared lib):** all three syntaxes (`$var`/`${var}`/`[[var]]`), each format hint
  (json/csv/singlequote/pipe), multi-value expansion, every built-in (`$__from/$__to/$__interval/$__range/
  ${__user.login}/${__dashboard}/${__workspace}/${__value}`), unknown-var-left-literal, and
  `interpolateArgs` over a nested JSON tree (the `argsTemplate` generalization — keep its existing cases
  green).
- **URL round-trip:** `?var-host=web01&var-host=web02&from=…&to=…&refresh=30s` parses to the selection,
  reload re-renders, multi-value repeats; malformed degrades to defaults (never throws).
- **Auto-refresh:** the timer re-resolves variables + re-runs sources on tick; `off` stops; changing the
  interval reschedules.
- **Live (real gateway):** `bus.publish` a JSON message → a `bus.watch` cell receives it over the new SSE;
  series.watch still works.
- **Widget config round-trip (real gateway):** add a cell → ⚙ → rename + change view/options → save →
  reload re-renders with the edits; the cell `title` persists.
- **JSON payload e2e (real gateway):** build `{text:"${newTodo}"}` → target a real write tool → assert the
  side effect (the row/sample/published message appears); the deny when the target is ungranted.
- **Shared-lib reuse:** an extension widget (proof-panel) reads `ctx.vars` and `ctx.timeRange` and
  interpolates a payload with the shared lib — proving extensions consume the same library.
- **Offline / sync + hot-reload:** the dashboard (with variables/titles) upserts idempotently on §6.8;
  uninstalling an extension evicts its variable-query sources and tears down `bus.watch` streams.

## Risks & hard problems

- **The vars library is a forever boundary the moment extensions link it.** Get `interpolate`/`interpolateArgs`
  + the `ctx.vars` shape right once; freeze with a `v`. A v2 is expensive. (Mirror the widget-builder v2
  discipline.)
- **`bus.publish` is fire-and-forget — don't let the JSON builder imply delivery.** A "send" that must arrive
  is an outbox effect, not a publish. The UI must not show a fake "delivered." State vs motion is load-bearing.
- **`bus.*` subject namespacing must be real.** If a caller can name `series/…` or another ws's subject, the
  wall leaks. Enforce the `ws/{id}/ext/` prefix host-side + a reserved-prefix denylist; test it bites a real
  publish.
- **Identity spoofing.** `${__user.*}`/`${__workspace}` resolved client/iframe-side is the classic hole — they
  MUST come from the token, shell-side, passed as resolved values. Test the iframe can't override them.
- **Refresh + live double-counting / thundering herd.** A 5s refresh across many cells + live watch can
  hammer the gateway; debounce, dedupe in-flight, and pause refresh when the tab is hidden (rubix-cube’s
  subscribe→snapshot pattern). Bound it.
- **Interpolation injection.** Never splice a variable into SQL text; bind as `vars`. The JSON builder
  substitutes into a parsed JSON value tree, not a string concat. Type-preserve (the `{{value}}` discipline).
- **Multi-value semantics.** Grafana's `${var:csv}` / `${var}` in different sinks (SQL `IN`, a JSON array, a
  URL repeat) behave differently; pick explicit format hints per sink and test each. Don't guess.
- **Variable dependency / cycles.** A query variable referencing another (`region` → `host`) needs ordered
  resolution; detect cycles, resolve in dependency order (Grafana does). Start with one level, name chained
  deps as a guarded follow-up.

## Open questions

Decided so the build has no blocker; residuals are named follow-ups.

- **Selected values: URL, not record** — DECIDED (Grafana parity, shareable, per-viewer). Definitions on the
  record.
- **One `query` type over `{tool,args}` for all sources** — DECIDED (SurrealDB/series/ext/Zenoh are tools).
- **Built-ins resolved shell-side from the token** — DECIDED (un-spoofable; passed into `ctx`).
- **`bus.publish`/`bus.watch` is the new API** — DECIDED; specced above. Residual: extract to `scope/bus/`
  or keep here? (Lean: ship here, extract if a second non-dashboard caller lands.)
- **Three syntaxes + format hints** — DECIDED to support all three refs + json/csv/singlequote/doublequote/
  pipe/raw; the richer multi-value forms (`{a,b}`, regex, glob) are a named follow-up.
- **`ctx` gains `vars`+`timeRange`** — DECIDED (additive, versioned); the shared lib is the contract.
- **Follow-ups (not v1):** ad-hoc filters (needs `store.schema` UI), chained/cascading variables beyond one
  level, the richer multi-value expansion forms, a `sql.generate`/AI assist for variable queries, and
  per-user variable defaults persisted server-side.

## Slice order (build)

1. **Slice 1 — widget settings/config** (`Cell.title` + the settings drawer). Smallest, unblocks "edit."
2. **The `vars` library** (`ui/src/lib/vars/`) — pure TS + unit tests; nothing else needs it yet but it's the
   spine.
3. **Slice 2 — variable model + bar + URL sync** (`Dashboard.variables`, `VariableBar`, the editor).
4. **Slice 3 — interpolation wired into cell calls + `ctx.vars`** (the payoff: panels re-point by variable).
5. **Platform fix — `bus.publish`/`bus.watch`** (the new verbs + gateway SSE).
6. **Slice 4 — auto-refresh + live events** (refresh picker + `bus.watch` wiring).
7. **Slice 5 — JSON payload builder** (uses the lib + the sink, incl. `bus.publish` and `todo.add`).

Each slice writes a session doc, promotes shipped truth to `public/frontend/dashboard.md`, and keeps the
mandatory deny + isolation tests green.

## Related

- [`widget-builder-scope.md`](widget-builder-scope.md) — the shipped v2 builder/bridge/cell this extends; the
  `argsTemplate` `{{value}}` (→ `${__value}`), `store.query` `{sql,vars}`, and source picker are the seeds.
- [`widget-palette-scope.md`](widget-palette-scope.md) — the editor (`mcp:dashboard.save:call`) gate reused
  for the ⚙ edit affordance.
- [`widgets-scope.md`](widgets-scope.md) / [`README.md`](README.md) — the dashboard subtopic index; add this
  to the read order.
- [`../routing-scope.md`](../routing-scope.md) — the shipped TanStack hash router + typed `?from`/`?to` this
  extends with `?var-<name>=` and `?refresh=`.
- [`../../extensions/reference-extensions-scope.md`](../../extensions/reference-extensions-scope.md) — the
  mqtt/zenoh/timescale bridges that consume `bus.*` and whose tools become variable sources/JSON targets.
- [`../../extensions/ui-federation-scope.md`](../../extensions/ui-federation-scope.md) — the widget `ctx`/bridge
  the resolved `vars`/`timeRange` ride on (the additive v2 ctx field).
- rubix-cube `frontend/lib/events/use-workspace-bus-stream.ts` (live SSE subscribe→snapshot),
  `frontend/lib/time.ts` (`convertToTimeParameters`/interval inference), `frontend/components/ui/date-range-filter/store.tsx`
  (URL-synced range) — the refresh/live/time patterns ported.
- Grafana docs (see header) — the variable types, built-ins, URL grammar, and syntax this matches.
- README **§3** (rules 3/5/6/7), **§6.1** (API shape), **§6.10** (jobs for unbounded variable queries),
  **§6.13** (extension UIs), **§7** (tenancy).
</content>
