# GenUI scope — AI-driven widgets over one renderer-agnostic generative-UI layer

Status: scope (the ask). Promotes to `public/genui/` once shipped.

We want a dashboard widget whose **layout is authored by the workspace agent** — the user types
"show me the pump-room flow counters next to today's ingest rate" and gets a real, live, grid-placed
widget — without betting the platform on any one generative-UI library. Two credible ecosystems
exist today ([Google A2UI](https://github.com/google/A2UI) — a stateful surface/patch *protocol*;
[Thesys OpenUI](https://github.com/thesysdev/openui) — a token-cheap streaming emission *DSL* +
React renderer). This scope defines **one reusable `@nube/genui` package** with a neutral internal
representation (IR) that any emission format sits behind, and the **`view:"genui"` dashboard
widget** as its first tenant. The layer is deliberately dashboard-independent so channels rich
responses, full pages, and extensions can reuse it — the same extraction move as
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
- **Renderer independence via a persisted IR.** One typed, versioned internal representation is
  what a cell stores and what the renderer consumes. Emission formats (OpenUI Lang now, A2UI JSONL
  when a consumer exists) are **authoring-time adapters** that parse *into* the IR; adding or
  swapping one never touches the render path, the cell contract, or consumers.
- **Reusable outside the dashboard.** `@nube/genui` imports nothing from `ui/src` — it takes a
  catalog, a bridge-shaped `call/watch` seam, and an IR + data model, and renders. The channel
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
  composes shipped surfaces: `agent.invoke`/`agent.watch`, `dashboard.save`, the widget bridge,
  the run stream. The **one** host-side change is a validation branch *inside* the existing
  `dashboard.save` handler for `view:"genui"` cells (Decision 6) — no new verb, cap, or table.
- **No A2UI *adapter* in v1.** The IR keeps A2UI's shape (below) — that decision costs nothing —
  but the JSONL parse/serialize adapter ships only when something actually speaks it (Flutter/Lit
  client, A2A interop, an import). Shipping two emission adapters to "prove the seam" inverts the
  point of an IR: the seam is proven by adapters being *addable* without touching consumers, not
  by shipping both on day one.
- **Not vendoring either library's styled components.** Foreign stylesheets (preflight, global
  utilities) are banned in `packages/*` — the catalog renders with our own shell-token-themed
  components.

## Intent / approach

**The key idea: standardize on an A2UI-*shaped*, versioned IR as the persisted contract, treat
emission formats as authoring-time adapters, and render through our own trusted catalog —
agent-authored *spec* is durable state on the cell; live *data* flows through the already-shipped
bindings.**

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

We implement this model **in-house** (`packages/genui/src/ir/`), keeping the message *shapes*
compatible with A2UI v0.9 so an adapter (or `@a2ui/web_core` itself) can be slotted in later — but
we do not depend on Google's packages now: the spec is pre-1.0 with active renames
(`beginRendering`→`createSurface`), npm packaging is still settling, and their renderer brings a
component pipeline we'd fight for CSS scoping. Same verdict shape as graphics-canvas: **patterns
adopted, dependency rejected** (for now; the adapter seam is exactly where it returns).

### Parse once, persist the IR — not the emission text

The emission format is optimized for *the model* (token cost, line-streamability); the IR is
optimized for *the platform* (typed, versioned, validatable). So the boundary is the **accept
step**: parse → `normalize` → `validate` happen **once**, loudly, when the author accepts the
preview — and what persists on the cell is the **typed IR**, never raw Lang/JSONL. Consequences,
each deliberate:

- The **render path carries no adapter and no normalize** — viewers mount `ir/` + `react/` only:
  deterministic, parser-free, and immune to emission-format churn (an A2UI v1.0 rename can at
  worst break an *authoring* adapter, never a persisted dashboard).
- **Sloppy generations fail at the author, not the viewer.** `normalize`'s fixes (dropped orphan,
  coerced prop, placeholder for an unknown component) are surfaced in the preview as warnings the
  author sees before saving; a spec that can't normalize to something sensible is rejected at
  accept — viewers never discover placeholders the author didn't.
- The raw emission text and prompt history are kept as **authoring metadata** (for refine turns
  and audit), not as the render input.

### The package — `packages/genui` (`@nube/genui`)

Standard `packages/*` layout (Vite lib mode + dts, `./style.css` export, react ≥18 peers),
FILE-LAYOUT folder-of-verbs. Two strata with different consumers:

**Render stratum (what every viewer loads):**

- `src/ir/` — the versioned types (`IrSpec` with a required `v`, `Surface`, `Component`,
  `DataModel`, `Patch`, `UiAction`) + pure ops: `applyPatch`, `resolveBindings` (JSON Pointer),
  `validate`. No parsers here.
- `src/catalog/` — `defineCatalog`: each entry `{name, description, props (JSON-Schema), render,
  deprecatedAliases?}`. From one catalog we generate **both** prompt surfaces: the OpenUI-style
  component signature prompt block and an A2UI-style catalog JSON. The catalog is the *only*
  thing the agent may instantiate — the constraint is structural, not prompt-hoped.
- `src/react/` — `<GenUiSurface spec data onAction/>`: walks the IR, dispatches to catalog
  `render` fns. CSS discipline per the package rules: everything under `.gu-root`, own `gu-*`
  classes, `--gu-*` tokens aliasing host shadcn vars with dark fallbacks, **no preflight**.

**Authoring stratum (loaded by the builder only):**

- `src/adapters/openui/` — parses OpenUI Lang → IR via **`@openuidev/lang-core`** (MIT, pure
  parser, no DOM/CSS — the one external dep this scope adds). Streaming: re-parse per
  `text-delta`, forward-refs resolve as lines land (the library's own model).
- `src/normalize/` — the LLM-sloppiness pass (unknown component → labeled placeholder + warning,
  dangling child id → dropped + warning, wrong-typed prop → coerced or defaulted + warning; never
  a blank panel, never a throw mid-stream). Runs during preview and at accept; **never at view
  time**.
- *(deferred)* `src/adapters/a2ui/` — A2UI v0.9 JSONL ⇄ IR, added as a one-file proof-of-seam
  when a real consumer appears.

**Versioning & catalog compatibility (the drift rule).** `IrSpec.v` names the IR schema version;
`ir/migrate.ts` upgrades old persisted specs forward on load (the same registry-drift class
`agent-config` handles explicitly). The catalog compat rule: a component or prop may not be
removed or renamed without leaving a `deprecatedAliases` entry in `defineCatalog` that maps the
old name/prop forward — CI asserts every component name referenced by the repo's spec fixtures
still resolves. A persisted cell must degrade only when its *grant* changes, never because the
catalog was refactored.

The v1 catalog is small and honest: layout (`stack`, `grid`, `card`), text/markdown, stat tile,
gauge, table, timeseries/bar/pie chart (wrapping the same chart primitives the shipped `views/*`
panels use), tag/badge, and `button`/`slider`/`switch` controls whose actions map to bridge tool
calls. Extensibility = pass a bigger catalog; extensions can contribute entries the same way they
contribute `[[widget]]` tiles today (follow-up slice).

### The widget — `view:"genui"`

- **Cell shape (additive):** `View` union gains `"genui"`; the cell persists
  `cell.options.genui = { v, ir, meta?: { format: "openui-lang", raw?, prompts? } }` — the typed
  IR is the render input; `meta` is authoring history for refine turns. One persistence path:
  the whole block is bounded (~8 KB) and **an over-budget spec is rejected at accept** with
  "simplify the widget" (an oversized catalog spec is almost certainly a bad generation;
  `render_template` spillover was considered and dropped — a second persistence path for the same
  artifact needs a lifecycle owner we don't want to invent on day one). Data targets are ordinary
  v3 `sources[]` (`Target`s with `refId`s) — **so the source picker, variables, transformations,
  and `viz.query` all apply unchanged.**
- **Render path:** `WidgetView` dispatches `genui` → `GenUiView`
  (`ui/src/features/dashboard/views/GenUiView.tsx`). Per the standing trust decision
  (`channels-rich-responses-scope.md`: AI-authored layout renders in the **sandboxed iframe
  tier**), v1 mounts `<GenUiSurface>` inside the shipped `WidgetIframe` runtime; the parent feeds
  it (a) the persisted IR and (b) data-model patches derived from `usePanelData` results keyed
  `/data/{refId}` (watch targets stream; the flow read path, the **existing deny/empty states**,
  and the `cell.source?.tool ? … : primaryTarget` empty-source guard are reused, not
  re-implemented — a denied target renders the same denied state every other panel shows, no
  genui-specific deny UX). Actions come back over the existing `bridge-call` postMessage seam,
  leashed to `cellTools(cell)` and re-capability-checked host-side per call — token never enters
  the frame.
- **In-process promotion is criteria-gated, not vibes-gated.** The iframe decision was made for
  AI-generated *code* (JSX templates); a catalog IR is *data* rendered by trusted components — a
  genuinely different threat model whose residual risks are enumerable. The promotion checklist,
  stated now so it can be satisfied mechanically:
  1. no catalog component uses `dangerouslySetInnerHTML` or renders a prop as a raw
     `href`/`src`/URL without sanitizing scheme+origin;
  2. markdown renders through the shell's sanitizing pipeline (no raw HTML pass-through);
  3. no prop is ever evaluated as code (no expression props, no template strings interpreted);
  4. every side effect goes through the leashed bridge (`cellTools` ∩ viewer grant, host
     re-check) — no direct `fetch`/DOM escape;
  5. CSS stays under `.gu-root` with no user-controlled class/style injection beyond token'd
     enum props.
  CI-testable items get tests; when all five hold, `GenUiView` mounts in-process (fixed-view
  tier) and the iframe tax (per-tick postMessage data patches, double React runtime) is dropped.
  Until then, iframe.
- **Authoring path:** the builder gains an "AI widget" entry. Prompt → `agent.invoke` under the
  **caller's** principal (`caller ∩ agent`, never wider) → `openRunStream(job)` → `text-delta`s
  fed through the Lang adapter → live preview renders progressively in the same iframe, normalize
  warnings shown inline → **accept runs parse/normalize/validate/size-check once, loudly** → the
  cell (IR + `meta` + the `sources[]` the agent chose) is written via the normal `dashboard.save`.
  Refine = another turn with the stored raw emission (or the IR re-serialized) + a data-shape
  summary in context; the durable job transcript is the history.
- **Emission format:** the skill teaches the agent to emit **OpenUI Lang** (roughly half the
  tokens of JSON, line-oriented so partial output renders cleanly). `meta.format` names it so a
  future A2UI (or other) authoring adapter is additive.
- **Headless authoring — any MCP caller is a first-class author.** Because the widget is just a
  cell written through `dashboard.save` (rule 7: MCP is the universal contract), the shell builder
  is *one client*, not the gate. Any principal holding `mcp:dashboard.save:call` + the read caps
  for the sources it binds can create a genui widget over the existing paths: the gateway's
  `POST /mcp/call` (CLI, API-key machine principals per `auth-caps/api-keys-scope.md`), routed
  MCP over Zenoh, or a third-party agent driven via the `external-agent` ACP runtime whose only
  tools are this same caps-checked surface. Headless callers skip the streaming preview and emit
  the **typed IR directly** (no Lang round-trip needed); the `genui-widget` skill documents both
  choreographies. Headless writers get the **same loud rejection** the shell gives, because
  `dashboard.save` structurally validates `options.genui` for `view:"genui"` cells (Decision 6):
  a malformed genui cell is refused at write time, not degraded at view time. The renderer's
  `validate` + placeholder pass stays as view-time defense-in-depth.

### The skill

The agent only does this well if taught. The implementing session writes
**`docs/skills/genui-widget/SKILL.md`** (grounded in a live run, per ABOUT-DOCS) and seeds it as a
core skill (`skill:core.genui-widget`, the two-tier catalog of `skills/core-skills-scope.md`),
grant-gated like any skill. Content: the catalog signatures, the emission format, and the
**data-discovery choreography**: enumerate candidates with `flows.list`/`flows.nodes`/
`series.find`/`store.schema`, sample with `flows.node_state`/`series.read`/`store.query`, then
bind `sources[]` exactly as the source-picker entries would (including the
`__flowNode`/`__flowPort` arg convention), and emit JSON-Pointer bindings against `/data/{refId}`.

**The codegen chain is named, because it crosses three build systems.** The catalog signature
block is **generated, not hand-written**: a `packages/genui` build step (`pnpm --filter
@nube/genui gen:skill`) renders it from `defineCatalog` into the marked section of
`docs/skills/genui-widget/SKILL.md`; the Rust node build then embeds that file as
`skill:core.genui-widget` (the core-skills seed path). CI runs the generator and fails on a dirty
diff — so the node can never embed a skill that lags the catalog. Hand-edits live only outside
the generated markers.

**Design-time data egress is a stated posture, not an accident.** Sampling real rows into the
agent's context is the moment workspace data reaches the configured model provider. Access is
already correct (`caller ∩ agent`, ai-gateway audit trail), and the skill bounds *volume*: the
choreography is `store.schema`/descriptors first, then **first-N rows (N small, default ≤20) of
only the candidate sources** — never table dumps. Workspaces with a local-only provider policy
get design-time privacy for free via the existing gateway routing.

### Rejected alternatives

- **Persist the raw emission text, adapters in the render path.** (The first draft of this scope
  did this.) Rejected: every viewer re-parses and re-normalizes forever, the render bundle
  carries parsers, silent normalize fixes surface as placeholders to viewers instead of warnings
  to the author, and emission-format churn (A2UI pre-1.0 renames) could break persisted
  dashboards. Parse once at accept; persist the typed, versioned IR.
- **Ship both emission adapters on day one.** Rejected: the A2UI direction had no named consumer,
  a hand-rolled incremental JSONL parser doubles the risky surface, and the IR→A2UI serializer
  had no consumer at all. The IR *shape* stays A2UI-compatible; the adapter lands with its first
  real speaker.
- **Adopt one library wholesale (either one).** OpenUI's renderer/UI kit couples us to its CSS
  and its language as *the* contract; A2UI's renderer is pre-1.0 with an unsettled packaging
  story and no emission ergonomics. Both remain one adapter away; neither becomes load-bearing.
- **Agent-in-the-loop rendering** (agent streams UI per view/refresh). Violates state-vs-motion
  (the widget's identity is durable state), costs tokens per glance, dies offline, and puts a
  model between a user and their data. The agent authors; bindings serve.
- **A bespoke `ai` cell payload with its own read path.** Re-implements `sources[]`/
  `usePanelData`/variables in parallel — exactly the fork rule 9 exists to prevent.
- **`render_template` spillover for large specs.** Rejected for v1: nothing owns deleting the
  template when the cell is replaced/removed, so it leaks orphans quietly — the exact failure the
  size bound exists to prevent, moved one table over. Reject oversized specs at accept instead.
- **JSX `template` generation instead of a catalog IR** (the existing scripted view). Kept as-is
  for power users, but as the *AI* target it's worse: arbitrary code needs the heaviest sandbox
  forever, can't be validated/normalized structurally, can't be patched incrementally, and can't
  be ported off React. The catalog IR is data.

## How it fits the core

- **Tenancy / isolation:** nothing new holds data — specs live inside `dashboard:{id}` cells
  (workspace-scoped, shipped), generation runs are workspace-walled jobs, and every data read
  goes through already-walled verbs. Isolation is inherited, and tested anyway.
- **Capabilities:** rendering needs only what the cell's tools already need (leash =
  `cellTools(cell)` ∩ viewer grant, host-re-checked per call). Authoring needs
  `mcp:agent.invoke:call` + `mcp:agent.watch:call` + `mcp:dashboard.save:call` + the skill grant.
  The agent's data reach during design is `caller ∩ agent` — it can never bind a source its
  invoking user couldn't read. Deny path: no `dashboard.save` → no "AI widget" builder entry
  (palette-gate precedent); denied source at view time → the standard `usePanelData` denied/empty
  state, not a crash and not a genui-specific rendering.
- **Placement:** either. Solo/edge node with a local provider authors offline; a cell authored
  anywhere renders anywhere. No `if cloud`.
- **MCP surface / API shape:** **consumes only** (`agent.invoke`, `agent.watch` SSE,
  `dashboard.save`, and the data verbs above). No new verbs — CRUD is the dashboard's, the live
  feed is the run stream + `series.watch`/`bus.watch`, batch N/A. The only host-side code change
  is a `view:"genui"` validation branch *inside* the existing `dashboard.save` handler
  (Decision 6): same verb, same cap, same table.
- **Data (SurrealDB):** state = the cell (`options.genui`: versioned IR + authoring meta,
  size-bounded at accept). No new tables, no second persistence path.
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
- **Skill doc:** required (drivable surface) — `docs/skills/genui-widget/SKILL.md`, named above,
  with its generator in the `@nube/genui` build and a CI freshness gate.

## Example flow

1. Alice (has `dashboard.save`) adds a widget → "AI widget" → types *"counter from the demo flow
   next to a 24 h chart of series `office/temp`, red when the counter stalls"*.
2. The shell calls `agent.invoke` (her principal); the run job starts; the shell opens
   `/runs/{job}/stream`.
3. The agent activates `skill:core.genui-widget`; calls `flows.list` → `flows.nodes` →
   `flows.node_state` (finds the counter node/port), `series.find` → `series.read` (samples ≤20
   rows to learn the shape) — each call capability-checked under `caller ∩ agent`.
4. It streams OpenUI Lang: a `grid` with a `stat` bound to `/data/A/value`, a `timeseries` bound
   to `/data/B/rows`, a threshold prop for the stall rule. The adapter re-parses per delta; the
   preview iframe fills in progressively; a normalize warning ("dropped dangling child `s3`")
   shows inline.
5. Alice accepts. Parse/normalize/validate/size-check run once, loudly; the shell persists the
   cell: `view:"genui"`, `options.genui.{v, ir, meta}`,
   `sources: [A: flows.node_state{…__flowNode/__flowPort}, B: series.watch{series:"office/temp"}]`
   via `dashboard.save`.
6. Steady state: `usePanelData` resolves A and B (B live over SSE); results patch the surface data
   model at `/data/A`, `/data/B`; the renderer re-binds the IR — **no agent, no model, no parser,
   no new caps**.
7. Bob (viewer, lacks the flow read cap) opens the dashboard: B renders; A shows the same
   `usePanelData` denied state every other panel shows. Workspace W2 never sees the cell at all.

## Testing plan

Per `scope/testing/testing-scope.md` — real store, real bus, real gateway, real agent loop; the
**only** permitted fake is the model provider behind the existing `MockProvider` seam (a true
external), scripted to emit fixed Lang fixtures.

- **Package unit (vitest, `packages/genui`):** Lang→IR round-trips; streaming partial-input
  renders (mid-line, forward refs); `normalize` on sloppy output (unknown component → placeholder
  **+ warning**, bad pointer → empty binding + warning) — the graphics-canvas
  validate-and-placeholder pattern; `applyPatch`/`resolveBindings`/`migrate` purity; **IR `v`
  migration golden files**; **catalog-compat gate** (every fixture component name resolves,
  including through `deprecatedAliases`); catalog prompt/JSON generation golden files; **skill
  generator freshness** (generated block matches `defineCatalog`, dirty diff fails).
- **Capability deny (mandatory):** `agent.invoke` denied without its cap; a genui cell whose IR
  action references a tool outside `cellTools` has the bridge call rejected host-side; builder
  entry hidden without `dashboard.save`; agent-under-Alice cannot bind a source Alice lacks;
  denied target renders the standard `usePanelData` denied state.
- **Workspace isolation (mandatory):** dashboard + run stream from W1 invisible from a W2
  principal (gateway test, seeded real records).
- **Integration (`pnpm test:gateway`):** author-flow E2E with the scripted provider — invoke →
  stream → parse-at-accept → save → re-load → render **without the adapter loaded**; accept-time
  rejection paths (unparseable emission, over-8 KB spec) fail loudly with the stated messages;
  **round-trip regression for the empty-`source` v3 trap** on a genui cell (the known gateway
  placeholder-`source` bug class); live update E2E — a real flow run bumps `flows.node_state`,
  the surface data model patches.
- **Offline/degrade:** cell renders from persisted IR with no provider configured; refine
  affordance disabled with an honest message.

Anything that breaks logs a `docs/debugging/genui/<symptom>.md` entry + regression test, per the
session rules.

## Risks & hard problems

- **LLM emission quality is the product.** A sloppy spec must be caught at accept — normalize
  warnings in the preview + loud accept-time validation carry this; budget real iteration time
  against real providers, not just the scripted one.
- **Streaming parse of Lang.** Incremental parsing is library-provided (`lang-core`'s own model),
  but our preview must render *something* stable through every mid-stream state — the
  normalize-during-preview pass is doing real work per delta; profile it.
- **The 8 KB accept bound needs real-world calibration.** If well-formed multi-panel specs
  routinely exceed it, the answer is catalog/prompt guidance ("one widget, one job") before it is
  a second persistence path — revisit only with evidence.
- **Catalog/skill/IR drift.** Three coupled artifacts (catalog code, generated skill, persisted
  specs). The generator + CI freshness gate + `deprecatedAliases` + `migrate` are the mitigations;
  skipping any one of them reintroduces silent degradation.
- **A2UI pre-1.0 churn.** Now contained by design: persisted cells store our IR, so spec renames
  can only ever touch the (deferred) authoring adapter. Watch it anyway before building that
  adapter.
- **Iframe data-plumbing overhead (until promotion).** Large tables patched over postMessage per
  tick; decimation (`series-decimation-scope.md`) and patch coalescing are the mitigations, and
  the concrete promotion checklist above is the exit.

## Decisions (v1 — build these; no open questions)

Every prior open question is resolved below so the implementing session can build straight through.
Deferrals are explicit follow-ups with a named trigger, **not** decisions to make mid-build.

1. **Trust tier — iframe for v1.** Ship `GenUiView` in the sandboxed `WidgetIframe` tier
   (the standing `channels-rich-responses` decision). Build the catalog to *already satisfy* the
   five promotion-checklist items above (no `dangerouslySetInnerHTML`, sanitized markdown, no
   code-valued props, all effects via the leashed bridge, no style injection) and add the CI
   tests for them — but do **not** promote in this slice. In-process promotion is a follow-up
   whose trigger is "checklist tests green in CI + one perf datapoint showing the iframe data
   tax matters"; the session records which items already hold in its session doc.
2. **Refine-turn context — full re-emit.** On a refine turn, resend the stored raw emission
   (`meta.raw`) plus a one-paragraph data-shape summary into the agent's context and let it
   re-emit the whole spec; parse/normalize/validate at accept exactly as the first turn. Do
   **not** build IR patch-lines in v1 — the follow-up trigger is a measured pain point (specs
   large enough that full re-emit is slow/expensive), at which point `updateComponents`-style
   patches apply cleanly against the persisted IR.
3. **Channel tenant — out of scope for this slice.** `render:{view:"genui"}` in rich responses is
   a **follow-up**, tracked in `channels-rich-responses-scope.md`. This slice ships the package +
   the dashboard tenant only; the package is built dashboard-independent so the channel consumer
   is later glue, not a rebuild. Do not touch `features/channel/` in this session.
4. **Design-time sampling — first ≤20 rows per candidate, no policy knob in v1.** The skill bounds
   egress to `store.schema`/descriptors first, then at most 20 sampled rows per candidate source
   (never table dumps). Local-provider workspaces get design-time privacy via existing gateway
   routing. A per-workspace "design-time sampling off" policy knob is a **follow-up** for regulated
   tenants (trigger: a tenant asks for it); v1 does not add the knob.
5. **Data cadence — reuse the shipped panel cadence, no per-target controls.** `GenUiView` drives
   the surface data model from `usePanelData` exactly as other panels: watch targets stream and
   patch `/data/{refId}` on tick; non-watch targets resolve on `refreshKey`. Do **not** add
   per-target cadence controls in v1 (follow-up only if a real dashboard needs mixed rates).
6. **Host-side IR validation on save — build it in this slice.** `dashboard.save` structurally
   validates `options.genui` when `view:"genui"`: IR schema `v` present and known, size within the
   ~8 KB bound, and every `component` name resolves in the catalog JSON (the generated artifact,
   not the TS). This closes the headless-MCP-author gap from day one — any MCP caller
   (`POST /mcp/call`, routed Zenoh, `external-agent`) gets the same loud rejection the shell gives,
   so a malformed genui cell is refused at write time, not degraded at view time. The view-time
   `validate`/placeholder pass stays as defense-in-depth. This is the **one** host-side addition in
   the slice and it adds no new verb/cap/table — it's a validation branch inside the existing
   `dashboard.save` handler, gated on `view:"genui"`.

## Related

- `docs/scope/channels/channels-rich-responses-scope.md` — the standing "generative UI is one more
  sandboxed view" decision + the reserved `view:"a2ui"`-style slot this scope fills properly (and
  the seam the promotion case in "Risks/Render path" would go back through).
- `docs/scope/frontend/dashboard/` — `widget-builder-scope.md` (the v2 cell/view/bridge contract),
  `source-picker-package-scope.md` (the extraction precedent + the loaders the skill mirrors),
  `viz/` (the `sources[]`/`fieldConfig`/`viz.query` spine this rides).
- `docs/scope/frontend/graphics-canvas-scope.md` — the prior A2UI evaluation (patterns adopted,
  dependency rejected) this scope extends to widgets.
- `docs/scope/flows/dashboard-binding-scope.md` — the `flows.node_state`/`flows.inject` binding
  the example flow uses; `docs/scope/datasources/datasources-scope.md` — the future
  `federation.query` targets.
- `docs/scope/agent/agent-scope.md`, `docs/scope/agent-run/` (RunEvent stream),
  `docs/scope/skills/core-skills-scope.md` (where `skill:core.genui-widget` lives and the seed
  path the generated skill file feeds).
- README `§3` (rules 5/7/9), `§6.5` (host chokepoint); `docs/key-stack.md` → "Generative UI" row.
- External: [A2UI spec v0.9](https://a2ui.org/specification/v0.9-a2ui/) ·
  [OpenUI Lang](https://www.openui.com/docs/openui-lang/renderer) ·
  [`@openuidev/lang-core`](https://github.com/thesysdev/openui).
