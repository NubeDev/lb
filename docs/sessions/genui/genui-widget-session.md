# GenUI — AI-driven widgets over one renderer-agnostic generative-UI layer (session)

- Date: 2026-07-03
- Scope: ../../scope/genui/genui-scope.md
- Stage: post-S8 (building on the shipped dashboard v2/v3 cell contract, the agent loop, the
  widget iframe tier, and the core-skills seed) — branch `ce-node-wiring-v2`
- Status: in-progress

## Goal

Ship the `@nube/genui` package (renderer-agnostic generative-UI IR + catalog + React surface,
plus the OpenUI-Lang authoring adapter and normalize pass) and its first tenant, the
`view:"genui"` dashboard widget: an AI-authored, live, grid-placed widget whose **layout is
durable IR state on the cell** and whose **data flows through the already-shipped `sources[]`
bindings** — no model in the render path. Per the scope's decision-complete "Decisions (v1)".

## The decisions (v1) I am building to (no re-deciding)

1. **Iframe trust tier** for v1 (`WidgetIframe`); build the catalog to already satisfy the
   5-item promotion checklist + add CI tests, but do NOT promote this slice.
2. **Refine = full re-emit** (resend `meta.raw` + data-shape summary); no IR patch-lines in v1.
3. **Channel tenant deferred** — package built dashboard-independent; do not touch
   `features/channel/`.
4. **Design-time sampling ≤20 rows/candidate**, no policy knob in v1 (skill bounds it).
5. **Data cadence = shipped panel cadence** (`usePanelData`), no per-target controls.
6. **Host-side IR validation on save** — a branch inside the existing `dashboard.save` handler
   for `view:"genui"` cells: IR `v` present+known, size ≤ ~8 KB, every component name resolves
   in the generated catalog JSON. No new verb/cap/table.

## File plan

### `packages/genui` (`@nube/genui`) — standard packages/* layout (cloned from source-picker)

Render stratum (every viewer loads):
- `src/ir/types.ts` — versioned `IrSpec {v, surface, components, dataModel}`, `Component`,
  `DataModel`, `Patch`, `UiAction`, `Binding`.
- `src/ir/applyPatch.ts` — pure `applyPatch(spec, patch)` (createSurface|updateComponents|
  updateDataModel|deleteSurface).
- `src/ir/resolveBindings.ts` — pure JSON-Pointer resolve of `{path}` bindings against the
  data model.
- `src/ir/validate.ts` — pure structural validate (against a passed catalog) → warnings/errors.
- `src/ir/migrate.ts` — forward-migrate an old persisted `IrSpec.v` to current.
- `src/ir/index.ts` — barrel for the pure ops.
- `src/catalog/defineCatalog.ts` — `defineCatalog(entries)` → `Catalog` with
  `deprecatedAliases`, name resolution.
- `src/catalog/prompt.ts` — generate the OpenUI-style component-signature prompt block.
- `src/catalog/toJson.ts` — generate the A2UI-style catalog JSON (+ the name set the host
  validates against).
- `src/catalog/library.ts` — build the `lang-core` `createLibrary(...)` from the catalog
  (drives the streaming parser schema).
- `src/catalog/nubeCatalog.tsx` — the v1 catalog (stack/grid/card, text/markdown, stat, gauge,
  table, timeseries/bar/pie, tag/badge, button/slider/switch). Satisfies the promotion
  checklist (no dangerouslySetInnerHTML, sanitized markdown, no code props, effects via bridge,
  no style injection).
- `src/react/GenUiSurface.tsx` — `<GenUiSurface spec data onAction/>` walks the IR.
- `src/react/GenUiContext.tsx` — the bridge-shaped `call/watch` + data context (imports nothing
  from ui/src).
- `src/genui.css` — scoped under `.gu-root`, `--gu-*` tokens aliasing host shadcn vars, no
  preflight.

Authoring stratum (builder only, separate entry so viewers never bundle it):
- `src/adapters/openui/parse.ts` — `parseLang(text, catalog) → {ir, warnings}` via
  `@openuidev/lang-core` `parse`.
- `src/adapters/openui/stream.ts` — `createLangStream(catalog)` wrapping
  `createStreamingParser` (re-parse per delta → IR).
- `src/adapters/openui/toIr.ts` — pure `ElementNode`-tree → flat id-referenced IR map.
- `src/normalize/normalize.ts` — LLM-sloppiness pass (unknown component→placeholder+warning,
  dangling child→drop+warning, wrong-typed prop→coerce/default+warning).
- `src/authoring.ts` — authoring entry barrel (parse + normalize + validate + size-check =
  `acceptSpec`).

Build/codegen:
- `src/bin/gen-skill.ts` — `pnpm --filter @nube/genui gen:skill`: render the catalog signature
  block from `defineCatalog` into the marked section of `docs/skills/genui-widget/SKILL.md`,
  and emit the catalog JSON the host embeds.
- `scripts/gen-catalog-json.ts` — emit `rust/crates/host/src/dashboard/genui_catalog.json`
  (checked in, host `include_str!`s it).

### `ui/` — the `view:"genui"` widget tenant

- `ui/src/features/dashboard/views/genui/GenUiView.tsx` — dispatcher: mounts `<GenUiSurface>`
  in the shipped `WidgetIframe`, feeds it `/data/{refId}` patches derived from `usePanelData`
  per target, actions back over `bridge-call`.
- `ui/src/features/dashboard/views/genui/genuiData.ts` — build the `{ [refId]: rows }` data
  model from each target's `usePanelData` (reuses the empty-source guard).
- `ui/src/features/dashboard/builder/AiWidget.tsx` — the "AI widget" authoring entry
  (prompt → `agent.invoke` → run stream → Lang adapter → live preview → accept → `dashboard.save`).
- Edits: `WidgetView.tsx` (genui branch), `dashboard.types.ts` (View union),
  `editor/VizPicker.tsx` + `viewOptions.ts` + `defaultCell` (make it pickable).

### Rust host — the one backend change (Decision 6) + skill seed

- `rust/crates/host/src/dashboard/genui.rs` — `check_genui_cells(&[Cell])` (mirrors `bounds.rs`).
- `rust/crates/host/src/dashboard/genui_catalog.json` — generated catalog name-set (include_str!).
- Edits: `dashboard/save.rs` (call after `check_cells_bounds`), `dashboard/mod.rs` (`mod genui;`).
- `docs/skills/genui-widget/SKILL.md` — auto-embedded + seeded as `skill:core.genui-widget`.

## What changed

(filled as work lands)

## Tests

(pasted green output as it lands)

## Decisions / notes

- `@openuidev/lang-core` verified installable (registry reachable; 0.2.7). Parser API:
  `createStreamingParser(library.toJSONSchema(), root).push(chunk) → ParseResult{root:ElementNode|null, meta}`.
  `ElementNode` nests via element-valued props (no `children[]` field) — the adapter walks props
  recursively to build the flat IR map. This is the one external dep the scope authorizes.
- `usePanelData` keys data by the whole `Cell` (not `/data/{refId}`), so `GenUiView` builds the
  refId→rows map itself by resolving each `sources[]` target — the scope's `/data/{refId}`
  convention lives in `genuiData.ts`, not in `usePanelData`.

## Open questions / scope updates

Scope was decision-complete; no re-decisions. Updates recorded here if reality forces one.
