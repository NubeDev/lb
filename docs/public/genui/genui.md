# GenUI — AI-authored dashboard widgets over one renderer-agnostic generative-UI layer (as built)

Scope: `../../scope/genui/genui-scope.md` · Session: `../../sessions/genui/genui-widget-session.md`

A dashboard widget whose **layout is authored by the workspace agent** from a natural-language
request, rendered live from a **persisted, versioned IR** — with **no model in the render path**.
Built as one reusable package (`@nube/genui`) with the `view:"genui"` dashboard widget as its first
tenant, plus one host-side validation branch and one core skill. Zero new core verbs/caps/tables.

## The package — `@nube/genui`

Standard `packages/*` layout (Vite lib, dts, `./style.css`, react ≥18 peers). **Two strata, two
entries** so a viewer never bundles the parser:

- **Render stratum** (`@nube/genui`, ~24 KB) — every viewer loads it; parser-free, deterministic:
  - `ir/` — the A2UI-*shaped*, versioned `IrSpec { v, surface, components, dataModel }` (flat
    id-referenced component map; JSON-Pointer `{$bind}` data bindings; typed patch messages) + pure
    ops `resolveBindings` / `applyPatch` / `validate` / `migrate`. Implemented in-house — **no
    dependency on Google's A2UI packages** (patterns adopted, dependency rejected; the adapter seam
    is where it would return).
  - `catalog/` — `defineCatalog` (each entry `{name, description, props, render, deprecatedAliases}`);
    the v1 catalog (stack/grid/card, text/markdown, stat, gauge, table, timeseries/barchart/piechart,
    tag(+`badge` alias), button/slider/switch). One catalog generates **both** prompt surfaces
    (component-signature block + A2UI-style catalog JSON) and the React render fns.
  - `react/` — `<GenUiSurface spec data catalog bridge onAction/>` walks the IR; CSS scoped under
    `.gu-root`, `--gu-*` tokens aliasing host shadcn vars, **no preflight**.
- **Authoring stratum** (`@nube/genui/authoring`) — builder only; the ONE place the single external
  dep `@openuidev/lang-core` loads:
  - `adapters/openui/` — OpenUI Lang (statement-based, `root = Stack("vertical", [a, b])`) → IR,
    one-shot + streaming (re-parse per `text-delta`, forward-refs resolve as lines land).
  - `normalize/` — the LLM-sloppiness pass (unknown→placeholder, dangling child→drop, wrong prop→
    coerce; each with a warning; never throws).
  - `authoring.acceptLang`/`acceptIr` — parse→normalize→validate→size-check **once, loudly**;
    `GENUI_MAX_BYTES = 8 KB`.

**Parse once, persist the IR.** The accept step runs the whole pipeline once; what persists on a
cell is the **typed IR**, never raw Lang. The render path carries no adapter and no normalize — an
emission-format rename can at worst break an authoring adapter, never a persisted dashboard.

## The widget — `view:"genui"`

- **Cell shape (additive):** the `View` union gains `"genui"`; the cell persists
  `options.genui = { v, ir, meta? }` (typed IR = render input; `meta` = authoring history).
- **Render:** `WidgetView` → `GenUiView` mounts `<GenUiSurface>` **in-process** (see "Trust tier"),
  feeding it a `/data/{refId}` model derived from the cell's ordinary v3 `sources[]`, each resolved
  through the shipped `usePanelData` (so source-picker, variables, transformations, watch/refresh
  cadence, and the deny/empty states are all inherited). The empty-source v3 guard is reused, not
  forked. Actions go over the `makeWidgetBridge(cellTools(cell))` leash, host-re-checked per call.
- **Authoring:** the builder's "AI widget" tab — prompt → `agent.invoke` (caller's principal, under
  `skill:core.genui-widget`) → run stream → live Lang preview → **accept** (parse/normalize/validate/
  size-check once) → normal `dashboard.save`. Headless MCP callers skip the stream and emit the typed
  IR directly.

### Trust tier — in-process (v1)

The scope's original "sandboxed iframe" decision was **amended during build** (with the scope
owner's approval): the shipped `WidgetIframe` sandbox cannot host a React surface (no import map, CSP
`connect-src 'none'`, eval'd non-React engines). A catalog IR is **trusted DATA rendered by our own
components**, genui widgets are **admin-authored** (the `dashboard.save` cap is the trust gate), and
the catalog satisfies all five promotion-checklist items (no `dangerouslySetInnerHTML`, sanitized
markdown, no code-valued props, effects only via the leashed bridge, no style injection) — **enforced
by CI tests**. So v1 renders in-process, the promotion end-state the scope already anticipated. If an
untrusted tenant ever needs genui, the sandbox question returns — with a DOM-walker renderer or an
inlined bundle, not the current React-in-`allow-scripts` sandbox that cannot work.

## Host-side validation (the one backend change)

`dashboard.save` structurally validates every `view:"genui"` cell (`dashboard/genui.rs`, called after
`check_cells_bounds`): IR `v` present + known, `options.genui` ≤ 8 KB, every component name resolves
in the embedded `genui_catalog.json`, root defined. A malformed genui cell is **refused at write
time**, not degraded at view time — so any MCP author (`POST /mcp/call`, routed Zenoh,
`external-agent`) gets the same loud rejection the shell gives. No new verb, cap, or table.

## The skill + codegen chain

`docs/skills/genui-widget/SKILL.md` (seeded as `skill:core.genui-widget`) teaches both authoring
choreographies (shell-streamed Lang + headless direct-IR) and the data-discovery choreography
(`flows.*`/`store.schema`/`series.find` → sample ≤20 rows → bind `sources[]` with the
`__flowNode`/`__flowPort` convention → JSON-Pointer bindings against `/data/{refId}`). Its
catalog-signature block is **generated** from `defineCatalog` by `pnpm --filter @nube/genui gen:skill`
(which also emits the host's `genui_catalog.json`); a CI freshness gate (a package test) fails on a
dirty diff, so the node never embeds a skill or validator that lags the catalog.

## Tests

Package unit (42, vitest) — Lang→IR round-trips + streaming, IR-op purity + migration goldens,
normalize, accept rejections, catalog-compat gate + deprecatedAliases, prompt/JSON goldens, the
promotion checklist, the gen:skill freshness gate. Host (8, cargo) — the accept/reject matrix +
capability-DENY + workspace-ISOLATION. UI unit (genui data helpers, incl. the empty-source v3 trap).
Gateway integration (4, real node) — save→reload→render-without-adapter, save-time rejection, the
empty-source v3 round-trip, and the save-cap deny. One bug fixed with a regression test
(`../../debugging/genui/genui-probe-setstate-in-render.md`).

## Deferred (explicit follow-ups, named triggers)

- **A2UI JSONL adapter** — lands with its first real speaker (Flutter/Lit client, A2A interop).
- **Channel `render:{view:"genui"}` tenant** — later glue (the package is dashboard-independent).
- **IR patch-line refine** — when full re-emit is measurably slow on large specs.
- **Design-time sampling policy knob** (regulated tenants), **catalog-from-extensions**, and
  **per-target cadence** — each trigger-gated in the scope.
