# Frontend scope — extract the source picker into a reusable package (`@nube/source-picker`)

Status: **scope (the ask)** — 2026-07-02. Parent: the shipped dashboard source picker
([`widget-builder-scope.md`](widget-builder-scope.md), [`widget-palette-scope.md`](widget-palette-scope.md))
and the flow binding ([`../../flows/dashboard-binding-scope.md`](../../flows/dashboard-binding-scope.md)).
Promotes to `public/frontend/` once shipped.

## The ask

Let a user (and an AI) **select a value/source from the DB, datasources, Zenoh (live series), and
flows — the same way the dashboard does** — from surfaces OTHER than the dashboard (first customer:
the `thecrew` graphics-canvas extension, so a scene shape can bind to a real source through a picker
instead of a hand-typed series name). The picking machinery already exists and unifies all of these,
but it lives *inside* `ui/src/features/dashboard/` and imports app-internal API clients, so nothing
outside the shell — least of all a separately-built extension — can reuse it.

**Extract it into a reusable, transport-agnostic package.**

## What exists today (the machinery, already unified)

One model, `SourceEntry`, already covers every source the ask names
([`ui/src/features/dashboard/builder/sourcePicker.ts`](../../../../ui/src/features/dashboard/builder/sourcePicker.ts)):

| Source | `group` | resolves to `{tool,args}` / view |
|---|---|---|
| SurrealDB | `sql` | `store.query {sql}` |
| Series (history) | `series` | `series.read {series}` |
| Zenoh (live) | `live` | `series.watch {series}` |
| Datasources (federation) | via `useDatasourceList` | `federation.query {source,sql}` |
| Flows (node ports) | `flows` | `flows.node_state` (read) / `flows.inject` (write) |
| Extension widgets | `widget` | `view: ext:<id>/<widget>` |

- **Model:** `sourcePicker.ts` — `SourceEntry`, `seriesEntries`/`liveEntries`/`extensionEntries`/
  `extWidgetEntries`/`flowsEntries`/`sqlSourceEntry`, `buildSourceEntries`.
- **Loader hook:** `useSourcePicker(ws)` — `Promise.all` of `listSeries`/`listExtensions`/`listFlows`
  (+`getFlow`)/`listFlowNodes`, tolerant of per-source deny (empty group), re-keyed on `ws`.
- **Datasource roster:** `editor/tabs/useDatasourceList.ts` — built-ins + `listDatasources`.
- **UI:** `editor/tabs/QueryTab.tsx` (the `<select>` + groups + datasource dropdown) and
  `editor/tabs/FlowsQuerySection.tsx` (the flow→node→port sub-picker); the older
  `builder/WidgetBuilder.tsx` renders the same groups.
- **Consumers today (all inside `features/dashboard/`):** `WidgetBuilder`, `editor/tabs/QueryTab`,
  `vars/VariableEditor`, `builder/JsonPayloadField`, `DashboardView`.

## Why it isn't reusable yet

1. **App-internal imports.** The picker imports `@/lib/ingest/ingest.api`, `@/lib/ext/ext.api`,
   `@/lib/flows/flows.api`, `@/lib/datasources` (thin `invoke`/`/mcp/call` wrappers) + `@/lib/dashboard`
   types. The `@/` alias resolves only in `ui/`. The shipped shared packages (`@nube/panel`,
   `@nube/nav-rail` in [`packages/`](../../../../packages/)) import **zero** `@/` — they are pure,
   props-driven, self-themed. A picker package must obey the same rule.
2. **Extensions build standalone.** `thecrew/ui` has its own lockfile and builds `--ignore-workspace`;
   it cannot `import` from `ui/src/`. A `workspace:*` package it declares as a dep is the only sharing
   path (same as it would consume `@nube/panel`).
3. **Transport differs.** The shell reaches the node via `@/lib/ipc/invoke` (Tauri or gateway HTTP);
   an extension reaches it via its **host-mediated bridge** (`bridge.call(tool,args)`). The package
   must not assume either.

## Intent / approach

**A headless-first, dependency-injected package.** The package owns the MODEL + the LOADER
orchestration + the (props-driven) UI; the HOST injects how to reach the node. Same shape as
`@nube/panel`: presentational + data-via-props, no `@/`, React as a peer dep.

```
@nube/source-picker  (packages/source-picker)
  model     SourceEntry, groups, buildSourceEntries(inputs) — PURE, no I/O (moved verbatim)
  loader    useSourcePicker(loaders, ws) — takes an INJECTED SourceLoaders object, not @/lib clients
  ui        <SourcePicker entries onSelect> + <FlowsSection> — props-driven, self-themed (scoped tokens)
  types     SourceLoaders (the injected fns), SourceSelection (the {tool,args}|view result)
```

- **The injected seam — `SourceLoaders`.** A small interface of the reads the picker needs:
  `listSeries()`, `listExtensions()`, `listFlows()`, `getFlow(id)`, `listFlowNodes()`,
  `listDatasources()`. Each returns the SAME shapes the package's types define (moved out of
  `@/lib/*`). The shell implements it by delegating to its `@/lib/*` clients (a ~10-line adapter); an
  extension implements it over its `bridge.call`. **The package never imports a transport.**
- **Types move with the model.** The picker's result vocabulary (`Source {tool,args}`, `Action
  {tool,argsTemplate}`, and the flow/ext/datasource row shapes) moves into the package as the
  canonical types; `@/lib/dashboard` re-exports or aliases them so the dashboard's `Cell`/`Target`
  keep compiling. The picker model does NOT depend on `Cell` — it already only produces
  `{tool,args}`/`view` (verified: `SourceEntry` carries `source?`/`action?`/`viewKey`, never a cell).
- **UI is optional to adopt.** Headless model+hooks are the must; the `<SourcePicker>` component is the
  fuller reuse. Ship both; the dashboard adopts the component (deleting its duplicated `<select>`),
  thecrew adopts the component too so both render the identical picker.

**Why headless-first + injected loaders (not "just move the files").** Moving the files as-is drags
`@/lib/*` + the Tauri/gateway transport into a package an extension can't build. Injection is what
makes ONE picker work from both the shell (gateway/Tauri) and an extension (bridge) — the whole point
of "reusable." It also makes the package testable with a fake loader object (no gateway) while the
real-path tests stay in each host.

**Rejected:** (a) *move to `ui/src/lib`* — shares within the shell but extensions still can't import
it (fails the actual ask). (b) *duplicate the picker in thecrew* — two copies drift (the exact trap
CLAUDE §8/§9 warn against). (c) *the package imports a transport* — welds it to one host; kills
extension reuse.

## How it fits the core

- **Zero core additions.** No new verb/cap/table/WIT. The package is pure frontend; it CONSUMES the
  same shipped reads (`series.*`, `ext.list`, `flows.*`, `datasource.list`) via injected loaders. The
  host still gates every call server-side; the picker is authoring-time UI only.
- **Capabilities/isolation:** unchanged — a denied loader read yields an empty group (honest), exactly
  as `useSourcePicker` does today; the workspace comes from the host's transport (token/bridge), never
  the picker.
- **Placement:** the package is transport-agnostic by construction — the shell injects gateway/Tauri,
  an extension injects its bridge; no `if cloud`, no role branch.
- **One responsibility per file (FILE-LAYOUT):** the package keeps the existing split — model
  (`sourcePicker.ts`), one loader hook, the `<select>` component, the flows sub-picker — one verb per
  file, no `utils`.
- **No mocks:** the dashboard's real-gateway suites (`widgetBuilder.gateway`, `panelEditor.gateway`,
  `flowsPanelEditor.gateway`) must stay green through the refactor (parity proof); the package's own
  unit tests use an injected fake-loader OBJECT (allowed — it's a pure function seam, not a fake
  backend; the real path is exercised by the host suites).

## Consumers (the migration)

1. **Dashboard first (parity refactor).** Rewire `useSourcePicker` onto the package (shell adapter for
   `SourceLoaders`), delete the duplicated model + `<select>` from `builder/`+`editor/tabs/`, keep every
   dashboard gateway/e2e test green. This proves the package reproduces shipped behavior before any new
   consumer.
2. **thecrew second (the new capability).** Wire the package into `thecrew/ui` with a bridge-backed
   `SourceLoaders`, so the scene property rail can bind a shape prop to a db/series/live/flow source
   through the SAME picker — replacing the current hand-typed `{channel: series}` with a real pick. (The
   scene `bind` stays `{channel}`; the picker just fills it — a follow-up may widen `bind` to the full
   `{tool,args}` if a scene needs store/flow sources directly.)

## Platform checklist (README §3)

- [x] **Workspace is the hard wall** — N/A to change: the picker is authoring-time UI; the workspace
  comes from the host transport (token/bridge), never the picker. Isolation is enforced + tested in the
  HOST suites (dashboard/thecrew gateway), unchanged.
- [x] **Capability-first** — no new grant. Each injected loader read is the shipped, gated verb
  (`series.*`/`ext.list`/`flows.*`/`datasource.list`); a denied read → an empty group (honest), exactly
  as today. The picker offers only what the loaders return.
- [x] **Symmetric nodes** — the package is transport-agnostic by construction (injected `SourceLoaders`);
  no `if cloud`. Shell injects gateway/Tauri; extension injects its bridge.
- [x] **One datastore** — N/A: pure frontend, no persistence.
- [x] **No mocks / no fake backend** — the package's unit tests use an injected fake **loader object**
  (a pure function seam, permitted — NOT a fake backend/`*.fake.ts`). The real store/gateway path stays
  proven by the host suites (dashboard gateway tests unchanged; thecrew gateway adds the pick→bind case).
- [x] **State vs motion** — unchanged: the picker just *labels* sources; `series.read` (state/history) vs
  `series.watch` (motion/live) stay distinct entries, as shipped.
- [x] **Stateless** — the package holds no durable state (pure render of loader results + selection).
- [x] **MCP is the contract** — the picker produces `{tool,args}`/`view`; every source IS an MCP tool.
- [x] **API shape** — N/A: consumes shipped reads (get/list) via loaders; exposes no verb.
- [x] **Durability** — N/A (authoring UI, no effects).
- [x] **One responsibility per file** — keep the existing split (model / loader hook / `<select>` /
  flows sub-picker), one verb per file, no `utils`.
- [x] **SDK/WIT impact** — none.
- [x] **Skill doc** — N/A. This is a frontend package refactor, not a new agent-/API-drivable surface;
  it consumes verbs already covered by `docs/skills/{store-read,ingest-series,dashboard-mcp,graphics-canvas}`.
  No new SKILL.md.

## Testing plan

Per `scope/testing/testing-scope.md`:

- **Parity (the headline):** all shipped dashboard picker tests stay green after the refactor —
  `widgetBuilder.test`, `widgetBuilder.gateway`, `panelEditor.gateway`, `flowsPanelEditor.gateway`,
  the QueryTab/FlowsQuerySection unit tests. No behavior change; the picker just lives elsewhere.
- **Package unit:** `buildSourceEntries` mapping (each group → `{tool,args}`/view), `useSourcePicker`
  with an injected fake loader (per-source deny → empty group; ws re-key), the `<SourcePicker>`
  component renders every group + fires `onSelect`.
- **Capability-deny + workspace-isolation:** stay in the HOST suites (real gateway) — the package has
  no transport to deny; the dashboard/thecrew gateway tests own those mandatory categories.
- **thecrew integration:** the scene rail picks a series through the package and the bound shape renders
  the live value (thecrew gateway suite, real seeded series).
- **Federation/build:** the package builds ESM+CJS+types+scoped CSS (like `@nube/panel`); thecrew's
  standalone `--ignore-workspace` build resolves it as a `workspace:*` dep.

## Risks & hard problems

- **Type gravity.** `@/lib/dashboard` is a big shared type home; moving the picker's result types out
  without breaking `Cell`/`Target` needs a careful re-export (alias, don't fork). Do it as the first
  step and keep `tsc` green at each move.
- **The flows sub-picker pulls `flowBinding` helpers** (`views/flowBinding.ts`) — decide whether those
  move into the package (they're about interpreting a flow `{tool,args}`, so likely yes) or stay host
  glue. Map the exact set before moving.
- **Two React copies.** React is a peer dep (as in `@nube/panel`); the shell provides one, and thecrew
  externalizes React to the shell import map — the package must not bundle React (same discipline the
  `federation-remote.preset` enforces).
- **Scope creep into the property rail.** thecrew's *rail* (how a picked source lands on a shape) is
  thecrew's concern; the PACKAGE only produces a selection. Keep the boundary at "returns a
  SourceSelection," don't absorb host-specific binding UI.
- **Big refactor, shipped code.** The dashboard picker is load-bearing (5 consumers, live e2e). Move in
  small green steps (types → model → loader → UI), running the gateway suites between steps.

## Open questions

1. **Package name:** `@nube/source-picker` (matches `@nube/panel`/`@nube/nav-rail`). Confirm.
2. **How much UI moves:** headless model+hooks only, or also the `<SourcePicker>`/`<FlowsSection>`
   components? (Leaning: both — the user asked for reuse "same as the dashboard," which is the UI too.)
3. **Does the scene `bind` widen** beyond `{channel: series}` to a full `{tool,args}`? — **DECIDED
   (2026-07-02): keep `bind = {channel: series}` for thecrew's first cut.** The picker offers every group
   (so the author sees the same surface as the dashboard), but for a scene bind thecrew maps the
   `series`/`live` selection to the channel (series name) and, for a non-series pick (db/flow/federation),
   surfaces an honest "not bindable to a scene prop yet" rather than silently dropping it. Widening `bind`
   to the full `{tool,args}` source vocab (so a shape prop reads the DB/a flow directly) is a real feature
   — scene-schema + renderer + validator + the bridge-source multiplexer all change — and is filed as its
   OWN follow-up scope (`scene-source-binding`), NOT smuggled into this package refactor. Rationale: the
   package is the reusable *picker*; how a host consumes a selection is the host's concern, and thecrew's
   scene model isn't ready for arbitrary sources this pass. Best long-term = one picker now, widen the
   scene bind deliberately later.

## Related

- [`widget-builder-scope.md`](widget-builder-scope.md), [`widget-palette-scope.md`](widget-palette-scope.md)
  — the shipped picker being extracted; [`widget-config-vars-scope.md`](widget-config-vars-scope.md).
- [`../../flows/dashboard-binding-scope.md`](../../flows/dashboard-binding-scope.md) — the flows read/write
  binding the picker's `flows` group produces.
- [`../graphics-canvas-scope.md`](../graphics-canvas-scope.md) + `rust/extensions/thecrew/` — the first
  new consumer (a scene shape binds through the picker).
- [`rules-as-source-scope.md`](rules-as-source-scope.md) — adds the `rules` group: a saved rule is a
  `rules.run` read source (Data Studio: query-with-a-rule → chart).
- `packages/panel`, `packages/nav-rail` — the shared-package pattern this follows (pure, props-driven,
  scoped tokens, React peer dep, ESM+CJS+dts+CSS build).
