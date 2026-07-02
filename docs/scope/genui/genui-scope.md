# GenUI scope — AI-driven widgets over one renderer-agnostic generative-UI layer

Status: scope (the ask). Promotes to `public/genui/` once shipped.

We want a dashboard widget whose **layout is authored by the workspace agent** — the user types
"show me the pump-room flow counters next to today's ingest rate" and gets a real, live, grid-placed
widget — without betting the platform on any one generative-UI library. Two credible ecosystems
exist today ([Google A2UI](https://github.com/google/A2UI) — a stateful surface/patch *protocol*;
[Thesys OpenUI](https://github.com/thesysdev/openui) — a token-cheap streaming emission *DSL* +
React renderer). This scope defines **one reusable `@nube/genui` package** with a neutral internal
representation that either library (or a home-grown emitter) sits behind, and the **`view:"genui"`
dashboard widget** as its first tenant. The layer is deliberately dashboard-independent so channels
rich responses, full pages, and extensions can reuse it — the same extraction move as
`@nube/source-picker`.

## Goals

- A user with dashboard edit rights describes a widget in natural language; the **shipped agent
  loop** (`agent.invoke` + the `RunEvent` SSE stream) designs it, previews it live while streaming,
  and persists it as a normal v2/v3 `Cell` on `dashboard.save`.
- The agent can **discover and bind real data** — flows (`flows.list`/`flows.nodes`/
  `flows.node_state`), the store (`store.query`), series (`series.find`/`series.read`/
  `series.watch`), installed extension tools, and (when the federation extension ships)
  `datasource.list`/`federation.query` — the exact same tools the `@nube/source-picker` loaders
  speak, taught by a granted **skill**.
- **Renderer independence.** One typed internal representation (IR); `a2ui` and `openui-lang`
  adapters both parse into it; the same IR renders through our own catalog renderer. Swapping or
  adding an emission format never touches the agent seam, the cell contract, or consumers.
- **Reusable outside the dashboard.** `@nube/genui` imports nothing from `ui/src` — it takes a
  catalog, a bridge-shaped `call/watch` seam, and a spec/stream, and renders. The channel
  rich-responses `view:"genui"` slot (already reserved by
  `channels/channels-rich-responses-scope.md`) is the named second tenant.
- **The agent designs; it does not serve.** Steady-state rendering reads through the existing
  gated bindings (`sources[]` → `usePanelData`/`useSource`/bridge `watch`). No model call per
  view, no token cost per refresh, works offline once authored.

## Non-goals

- **Not a new base render layer.** `channels-rich-responses-scope.md` already decided: generative
  UI is *one more sandboxed `view`*, never the substrate fixed views run through. This scope obeys
  that — deterministic tabular data keeps rendering through the in-process `table`/`timeseries`
  views untouched.
- **Not "agent in the hot path".** No per-render or per-refresh model calls; no live agent proxying
  of data reads.
- **Not a chat UI.** The channel/agent chat surface exists (`features/channel/`); this widget's
  prompt box is an authoring control, not a conversation product.
- **Not the graphics canvas.** Free-form scenes are `frontend/graphics-canvas-scope.md`; this is
  catalog-constrained widget/panel UI.
- **No new core verbs, capabilities, or tables** in v1 (the `source-picker` posture). Everything
  composes shipped surfaces: `agent.invoke`/`agent.watch`, `dashboard.save`, `template.put/get`,
  the widget bridge, the run stream.
- **Not vendoring either library's styled components.** Foreign stylesheets (preflight, global
  utilities) are banned in `packages/*` — the catalog renders with our own shell-token-themed
  components.

## Intent / approach

**The key idea: standardize on an A2UI-*shaped* IR as the internal contract, treat emission
formats as adapters, and render through our own trusted catalog — agent-authored *spec* is durable
state on the cell; live *data* flows through the already-shipped bindings.**

### Why the IR is A2UI-shaped

OpenUI Lang parses down to a component tree; A2UI's message model is strictly more general
(stateful surfaces, incremental patches, path-addressed data). A Lang document maps losslessly
into it; the reverse does not hold. So the IR adopts A2UI's four load-bearing patterns — the same
ones `graphics-canvas-scope.md` already adopted for scenes:

- **Surface** — one addressable render root (`surfaceId`), here: one cell.
- **Flat id-referenced component map** — `{id, component, props, children: [ids]}`; easy for an
  LLM to emit and patch incrementally, cheap to validate, orphan-tolerant.
- **JSON-Pointer data model per surface** — components *bind* paths (`{"path": "/data/A/latest"}`)
  instead of embedding values, so a data tick is a data-model patch, never a re-render of the tree.
- **Typed messages** — `createSurface | updateComponents | updateDataModel | deleteSurface` in;
  `action {surfaceId, componentId, name, context}` out.

We implement this model **in-house** (`packages/genui/src/ir/`), staying message-compatible with
A2UI v0.9 so `@a2ui/web_core`/`@a2ui/react` could be slotted in later — but we do not depend on
them now: the spec is pre-1.0 with active renames (`beginRendering`→`createSurface`), npm
packaging is still settling, and their renderer brings a component pipeline we'd fight for CSS
scoping. Same verdict shape as graphics-canvas: **patterns adopted, dependency rejected** (for
now; the adapter seam is exactly where it returns).

### The package — `packages/genui` (`@nube/genui`)

Standard `packages/*` layout (Vite lib mode + dts, `./style.css` export, react ≥18 peers),
FILE-LAYOUT folder-of-verbs:

- `src/ir/` — the types (`Surface`, `Component`, `DataModel`, `Patch`, `UiAction`) + pure ops:
  `applyPatch`, `resolveBindings` (JSON Pointer), `validate`, and **`normalize`** — the
  LLM-sloppiness pass (unknown component → labeled placeholder, dangling child id → dropped,
  wrong-typed prop → coerced or defaulted; never a blank panel, never a throw mid-stream).
- `src/catalog/` — `defineCatalog`: each entry `{name, description, props (JSON-Schema), render}`.
  From one catalog we generate **both** prompt surfaces: the OpenUI-style component signature
  prompt block and an A2UI-style catalog JSON. The catalog is the *only* thing the agent may
  instantiate — the constraint is structural, not prompt-hoped.
- `src/adapters/openui/` — parses OpenUI Lang → IR via **`@openuidev/lang-core`** (MIT, pure
  parser, no DOM/CSS — the one external dep this scope adds). Streaming: re-parse per
  `text-delta`, forward-refs resolve as lines land (the library's own model).
- `src/adapters/a2ui/` — parses A2UI v0.9 JSONL messages → IR (near pass-through) and serializes
  IR → A2UI messages (the export/interop direction).
- `src/react/` — `<GenUiSurface spec data onAction/>`: walks the IR, dispatches to catalog
  `render` fns. CSS discipline per the package rules: everything under `.gu-root`, own `gu-*`
  classes, `--gu-*` tokens aliasing host shadcn vars with dark fallbacks, **no preflight**.

The v1 catalog is small and honest: layout (`stack`, `grid`, `card`), text/markdown, stat tile,
gauge, table, timeseries/bar/pie chart (wrapping the same chart primitives the shipped `views/*`
panels use), tag/badge, and `button`/`slider`/`switch` controls whose actions map to bridge tool
calls. Extensibility = pass a bigger catalog; extensions can contribute entries the same way they
contribute `[[widget]]` tiles today (follow-up slice).

### The widget — `view:"genui"`

- **Cell shape (additive):** `View` union gains `"genui"`; the spec persists as
  `cell.options.genui = { format: "openui-lang" | "a2ui", spec, prompts?: [...] }` with a size
  bound (~8 KB); larger specs go to a durable `render_template:{id}` via the shipped
  `template.put`/`template.get` and are referenced by `options.genui.templateId`. Data targets are
  ordinary v3 `sources[]` (`Target`s with `refId`s) — **so the source picker, variables,
  transformations, and `viz.query` all apply unchanged.**
- **Render path:** `WidgetView` dispatches `genui` → `GenUiView`
  (`ui/src/features/dashboard/views/GenUiView.tsx`). Per the standing trust decision
  (`channels-rich-responses-scope.md`: AI-authored layout renders in the **sandboxed iframe
  tier**), v1 mounts `<GenUiSurface>` inside the shipped `WidgetIframe` runtime; the parent feeds
  it (a) the parsed spec and (b) data-model patches derived from `usePanelData` results keyed
  `/data/{refId}` (watch targets stream; the flow read path and the
  `cell.source?.tool ? … : primaryTarget` empty-source guard are reused, not re-implemented).
  Actions come back over the existing `bridge-call` postMessage seam, leashed to
  `cellTools(cell)` and re-capability-checked host-side per call — token never enters the frame.
- **Authoring path:** the builder gains an "AI widget" entry. Prompt → `agent.invoke` under the
  **caller's** principal (`caller ∩ agent`, never wider) → `openRunStream(job)` → `text-delta`s
  fed through the chosen adapter → live preview renders progressively in the same iframe → user
  accepts → the cell (spec + the `sources[]` the agent chose) is written via the normal
  `dashboard.save`. Refine = another turn with the current spec + data-shape summary in context;
  the durable job transcript is the history.
- **Emission default:** the skill teaches the agent to emit **OpenUI Lang** (roughly half the
  tokens of JSON, line-oriented so partial output renders cleanly) with A2UI JSONL as the
  documented alternate. Both adapters ship from day one — that's the whole point of the IR.

### The skill

The agent only does this well if taught. The implementing session writes
**`docs/skills/genui-widget/SKILL.md`** (grounded in a live run, per ABOUT-DOCS) and seeds it as a
core skill (`skill:core.genui-widget`, the two-tier catalog of `skills/core-skills-scope.md`),
grant-gated like any skill. Content: the catalog signatures (generated from `defineCatalog`, not
hand-copied — drift is the failure mode), the emission format, and the **data-discovery
choreography**: enumerate candidates with `flows.list`/`flows.nodes`/`series.find`/`store.schema`,
sample with `flows.node_state`/`series.read`/`store.query`, then bind `sources[]` exactly as the
source-picker entries would (including the `__flowNode`/`__flowPort` arg convention), and emit
JSON-Pointer bindings against `/data/{refId}`.

### Rejected alternatives

- **Adopt one library wholesale (either one).** OpenUI's renderer/UI kit couples us to its CSS and
  its language as *the* contract; A2UI's renderer is pre-1.0 with an unsettled packaging story and
  no emission ergonomics. Both remain one adapter away; neither becomes load-bearing.
- **Agent-in-the-loop rendering** (agent streams UI per view/refresh). Violates state-vs-motion
  (the widget's identity is durable state), costs tokens per glance, dies offline, and puts a
  model between a user and their data. The agent authors; bindings serve.
- **A bespoke `ai` cell payload with its own read path.** Re-implements `sources[]`/
  `usePanelData`/variables in parallel — exactly the fork rule 9 exists to prevent.
- **JSX `template` generation instead of a catalog IR** (the existing scripted view). Kept as-is
  for power users, but as the *AI* target it's worse: arbitrary code needs the heaviest sandbox
  forever, can't be validated/normalized structurally, can't be patched incrementally, and can't
  be ported off React. The catalog IR is data.

## How it fits the core

- **Tenancy / isolation:** nothing new holds data — specs live in `dashboard:{id}` /
  `render_template:{id}` (workspace-scoped, shipped), generation runs are workspace-walled jobs,
  and every data read goes through already-walled verbs. Isolation is inherited, and tested anyway.
- **Capabilities:** rendering needs only what the cell's tools already need (leash =
  `cellTools(cell)` ∩ viewer grant, host-re-checked per call). Authoring needs
  `mcp:agent.invoke:call` + `mcp:agent.watch:call` + `mcp:dashboard.save:call` + the skill grant.
  The agent's data reach during design is `caller ∩ agent` — it can never bind a source its
  invoking user couldn't read. Deny path: no `dashboard.save` → no "AI widget" builder entry
  (palette-gate precedent); denied source at view time → the bound component renders its
  honest denied/empty state, not a crash.
- **Placement:** either. Solo/edge node with a local provider authors offline; a cell authored
  anywhere renders anywhere. No `if cloud`.
- **MCP surface / API shape:** **consumes only** (`agent.invoke`, `agent.watch` SSE,
  `dashboard.save`, `template.put/get`, and the data verbs above). No new verbs — CRUD is the
  dashboard's, the live feed is the run stream + `series.watch`/`bus.watch`, batch N/A.
- **Data (SurrealDB):** state = the cell (`options.genui`) and optional `render_template` assets.
  No new tables.
- **Bus (Zenoh):** motion = the existing run-stream subject during authoring and the existing
  series/bus watch subjects at view time. Nothing new; fire-and-forget semantics unchanged.
- **Sync / authority:** a genui cell syncs as dashboard state does today. Authoring requires model
  access; rendering does not — offline behavior is "renders with last data, refine disabled",
  stated in the UI.
- **Secrets:** none touched. Model credentials stay in the ai-gateway; the iframe never sees a
  token (unchanged invariant).
- **Stateless extensions / hot-reload:** N/A in v1 (shell + package only); the catalog-from-
  extensions follow-up rides the existing manifest/grant machinery.
- **SDK/WIT impact:** none. Loudly: the WIT boundary is untouched.
- **One responsibility per file:** the package layout above is folder-of-verbs; `GenUiView.tsx`
  stays a dispatcher-sized file like its `views/*` siblings.
- **Skill doc:** required (drivable surface) — `docs/skills/genui-widget/SKILL.md`, named above.

## Example flow

1. Alice (has `dashboard.save`) adds a widget → "AI widget" → types *"counter from the demo flow
   next to a 24 h chart of series `office/temp`, red when the counter stalls"*.
2. The shell calls `agent.invoke` (her principal); the run job starts; the shell opens
   `/runs/{job}/stream`.
3. The agent activates `skill:core.genui-widget`; calls `flows.list` → `flows.nodes` →
   `flows.node_state` (finds the counter node/port), `series.find` → `series.read` (samples the
   shape) — each call capability-checked under `caller ∩ agent`.
4. It streams OpenUI Lang: a `grid` with a `stat` bound to `/data/A/value`, a `timeseries` bound
   to `/data/B/rows`, a threshold prop for the stall rule. The adapter re-parses per delta; the
   preview iframe fills in progressively.
5. Alice accepts. The shell persists the cell: `view:"genui"`, `options.genui.{format,spec}`,
   `sources: [A: flows.node_state{…__flowNode/__flowPort}, B: series.watch{series:"office/temp"}]`
   via `dashboard.save`.
6. Steady state: `usePanelData` resolves A and B (B live over SSE); results patch the surface data
   model at `/data/A`, `/data/B`; the iframe re-binds — **no agent, no model, no new caps**.
7. Bob (viewer, lacks the flow read cap) opens the dashboard: B renders; A shows its denied state.
   Workspace W2 never sees the cell at all.

## Testing plan

Per `scope/testing/testing-scope.md` — real store, real bus, real gateway, real agent loop; the
**only** permitted fake is the model provider behind the existing `MockProvider` seam (a true
external), scripted to emit fixed Lang/JSONL fixtures.

- **Package unit (vitest, `packages/genui`):** adapter→IR round-trips for both formats; streaming
  partial-input renders (mid-line, forward refs, orphaned JSONL message); `normalize` on sloppy
  output (unknown component → placeholder, bad pointer → empty binding) — the graphics-canvas
  validate-and-placeholder pattern; `applyPatch`/`resolveBindings` purity; catalog prompt/JSON
  generation golden files.
- **Capability deny (mandatory):** `agent.invoke` denied without its cap; a genui cell whose spec
  references a tool outside `cellTools` has the bridge call rejected host-side; builder entry
  hidden without `dashboard.save`; agent-under-Alice cannot bind a source Alice lacks.
- **Workspace isolation (mandatory):** dashboard + `render_template` + run stream from W1
  invisible from a W2 principal (gateway test, seeded real records).
- **Integration (`pnpm test:gateway`):** author-flow E2E with the scripted provider — invoke →
  stream → parse → save → re-load → render; **round-trip regression for the empty-`source` v3
  trap** on a genui cell (the known gateway placeholder-`source` bug class); live update E2E — a
  real flow run bumps `flows.node_state`, the surface data model patches.
- **Offline/degrade:** cell renders from persisted spec with no provider configured; refine
  affordance disabled with an honest message.

Anything that breaks logs a `docs/debugging/genui/<symptom>.md` entry + regression test, per the
session rules.

## Risks & hard problems

- **LLM emission quality is the product.** A sloppy spec must degrade to labeled placeholders,
  never a blank cell — `normalize` + the skill's few-shot examples carry this; budget real
  iteration time against real providers, not just the scripted one.
- **Two streaming parsers.** Incremental Lang parsing is library-provided; incremental JSONL is
  ours. Mid-stream invalid states must render *something* stable (buffer-until-root, per A2UI).
- **Spec size vs the cell record.** The 8 KB inline bound + template spillover needs enforcing at
  save, or dashboards bloat quietly.
- **Catalog/skill drift.** If the skill doc is hand-written it lies within a month — generate the
  signature block from `defineCatalog` and assert freshness in CI.
- **A2UI pre-1.0 churn.** The adapter pins v0.9 message names; a v1.0 rename lands as an adapter
  patch, not an IR change — that's the bet, watch it.
- **Iframe data-plumbing overhead.** Large tables patched over postMessage per tick; decimation
  (`series-decimation-scope.md`) and patch coalescing are the mitigations; measure early.

## Open questions

1. **In-process promotion.** The IR is declarative data rendered by *trusted* catalog code — is
   the iframe tier actually load-bearing here, or inherited caution? Proposal to resolve during
   implementation: ship v1 in the iframe (standing decision), then bring a promotion case to the
   rich-responses trust seam once the catalog is proven free of code-carrying components.
2. **Refine-turn context.** Resend the whole spec each turn vs teach the agent `updateComponents`
   patches (A2UI's strength; Lang lacks it — likely: full re-emit for v1, patches when it hurts).
3. **Channel tenant timing.** Same slice or follow-up for `render:{view:"genui"}` in
   rich-responses? (Recommend follow-up: the adapter from Item body → `WidgetView` already exists;
   it should be a page of glue.)
4. **Data-shape hints to the agent.** How much sampled data goes into context at design time
   (rows are big; `store.schema` + first-N sampling is probably enough)?
5. **Per-cell refresh vs surface patches.** Does `refreshKey` full-resolve stay acceptable, or do
   watch-driven partial `/data/{refId}` patches need distinct cadence controls per target?

## Related

- `docs/scope/channels/channels-rich-responses-scope.md` — the standing "generative UI is one more
  sandboxed view" decision + the reserved `view:"a2ui"`-style slot this scope fills properly.
- `docs/scope/frontend/dashboard/` — `widget-builder-scope.md` (the v2 cell/view/bridge contract),
  `source-picker-package-scope.md` (the extraction precedent + the loaders the skill mirrors),
  `viz/` (the `sources[]`/`fieldConfig`/`viz.query` spine this rides).
- `docs/scope/frontend/graphics-canvas-scope.md` — the prior A2UI evaluation (patterns adopted,
  dependency rejected) this scope extends to widgets.
- `docs/scope/flows/dashboard-binding-scope.md` — the `flows.node_state`/`flows.inject` binding
  the example flow uses; `docs/scope/datasources/datasources-scope.md` — the future
  `federation.query` targets.
- `docs/scope/agent/agent-scope.md`, `docs/scope/agent-run/` (RunEvent stream),
  `docs/scope/skills/core-skills-scope.md` (where `skill:core.genui-widget` lives).
- README `§3` (rules 5/7/9), `§6.5` (host chokepoint); `docs/key-stack.md` → "Generative UI" row.
- External: [A2UI spec v0.9](https://a2ui.org/specification/v0.9-a2ui/) ·
  [OpenUI Lang](https://www.openui.com/docs/openui-lang/renderer) ·
  [`@openuidev/lang-core`](https://github.com/thesysdev/openui).
