# GenUI — AI-driven widgets over one renderer-agnostic generative-UI layer (session)

- Date: 2026-07-03
- Scope: ../../scope/genui/genui-scope.md
- Stage: post-S8 (building on the shipped dashboard v2/v3 cell contract, the agent loop, the
  widget iframe tier, and the core-skills seed) — branch `ce-node-wiring-v2`
- Status: done

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

### `packages/genui` (`@nube/genui`) — new reusable package (standard packages/* layout)

Render stratum (`.` entry — every viewer loads; parser-free):
- `src/ir/{types,resolveBindings,applyPatch,validate,migrate,index}.ts` — the versioned IR + pure
  ops. `resolveBindings` = JSON-Pointer resolve of `{$bind}` props; `applyPatch` = the four A2UI
  messages; `validate` = structural check against a catalog; `migrate` = forward version upgrade.
- `src/catalog/{defineCatalog,toJson,prompt,library,nubeCatalog,charts,markdown}.ts(x)` — the
  catalog contract + the v1 `nubeCatalog` (stack/grid/card/text/markdown/stat/gauge/table/
  timeseries/barchart/piechart/tag(+badge alias)/button/slider/switch/placeholder). `toJson` +
  `prompt` are generated from `defineCatalog`. `library` builds the lang-core parser schema
  (lowercase catalog names ⇄ PascalCase Lang names).
- `src/react/{GenUiSurface,GenUiContext}.tsx` + `src/genui.css` — `<GenUiSurface>` walks the IR and
  dispatches to catalog render fns; CSS scoped under `.gu-root`, `--gu-*` aliasing host shadcn vars.

Authoring stratum (`./authoring` entry — builder only; the ONE place `@openuidev/lang-core` loads):
- `src/adapters/openui/{parse,stream,toIr}.ts` — OpenUI Lang → IR (one-shot + streaming). `toIr`
  lowers the parser's `ElementNode` tree (nests via element-valued props, no `children[]`) to the
  flat id-referenced IR map.
- `src/normalize/normalize.ts` — the sloppiness pass (unknown→placeholder+warning, dangling
  child→drop+warning, wrong-typed prop→coerce/default+warning; never throws).
- `src/authoring.ts` — `acceptLang`/`acceptIr` = parse→normalize→validate→size-check ONCE, loudly;
  `GENUI_MAX_BYTES = 8*1024`.

Codegen (`pnpm --filter @nube/genui gen:skill`):
- `src/bin/gen-skill.ts` — renders the catalog signature block into `docs/skills/genui-widget/
  SKILL.md` (between markers) AND emits `rust/crates/host/src/dashboard/genui_catalog.json`. A
  `--check` mode is the CI freshness gate.

### `ui/` — the `view:"genui"` tenant + AI-widget authoring

- `views/genui/GenUiView.tsx` — mounts `<GenUiSurface>` IN-PROCESS (Decision 1 amended, below), feeds
  it a `/data/{refId}` model from one `usePanelData` probe per `sources[]` target, actions over the
  `makeWidgetBridge(cellTools)` leash.
- `views/genui/genuiData.ts` — the refId→RefData shaping + the reused empty-source v3 guard.
- `builder/useGenUiAuthor.ts` — prompt → `agent.invoke` (skill `core.genui-widget`) → run stream →
  live Lang preview → durable answer → `acceptLang`.
- `editor/tabs/GenUiAuthorTab.tsx` — the "AI widget" options tab.
- Edits: `lib/dashboard/dashboard.types.ts` (View union += `genui`), `views/WidgetView.tsx` (genui
  branch), `editor/VizPicker.tsx` (pickable "AI widget"), `editor/cellEditorState.ts` (`genui` owned
  option key, round-trips), `editor/tabs/PanelOptionsTab.tsx` + `PanelEditor.tsx` (route + thread ws).

### Rust host — the one backend change (Decision 6)

- `crates/host/src/dashboard/genui.rs` — `check_genui_cells` (IR `v` known, ≤8 KB, every component
  name in the embedded `genui_catalog.json`, root defined). Called from `save.rs` after
  `check_cells_bounds`; `mod genui;` in `mod.rs`. No new verb/cap/table.
- `docs/skills/genui-widget/SKILL.md` — auto-embedded + seeded as `skill:core.genui-widget` by the
  existing `assets` build.rs / seed path (no Rust change needed to add a skill).

## Decision 1 amended (flagged to and approved by the scope owner)

The scope said "mount `<GenUiSurface>` in the shipped `WidgetIframe` sandbox". That sandbox provably
cannot host a React surface (no import map → bare `react` won't resolve; CSP `connect-src 'none'` +
no same-origin → can't load a bundle; engines run eval'd non-React code — see
`debugging/frontend/ext-widget-iframe-tier-cannot-resolve-bare-react.md`). I stopped and asked; the
owner's call: **in-process is fine** — genui widgets are admin-authored (the `dashboard.save` cap is
the trust gate), the catalog IR is trusted DATA rendered by our own components, and the 5 promotion-
checklist items are satisfied (CI-tested). So v1 renders in-process (the promotion end-state),
avoiding the double-React / per-tick-postMessage tax and keeping offline rendering. Scope Decision 1
updated with the reasoning; the "if an untrusted tenant needs genui, the sandbox question returns
with a DOM-walker or inlined bundle" note is recorded there.

## Tests (all green)

`@nube/genui` package — `../../ui/node_modules/.bin/vitest run` (8 files, 42 tests):
```
✓ src/ir/resolveBindings.test.ts (7)   ✓ src/ir/applyPatch.test.ts (6)
✓ src/normalize/normalize.test.ts (5)  ✓ src/catalog/catalog.test.ts (5)
✓ src/react/checklist.source.test.ts (4)  ✓ src/react/checklist.test.tsx (6)
✓ src/adapters/openui/adapter.test.ts (5)  ✓ src/authoring.test.ts (4)
 Test Files  8 passed (8)   Tests  42 passed (42)
```
Covers: Lang→IR round-trips + streaming/forward-refs; resolveBindings/applyPatch/migrate purity +
migration goldens; normalize (unknown/dangling/coerce); accept rejection paths (unparseable,
over-8 KB); catalog-compat gate + deprecatedAliases; prompt/JSON goldens; the promotion checklist
(1 no dangerouslySetInnerHTML, 2 markdown sanitizes + non-http link dropped, 3 no eval/new Function
source scan, 4 controls emit via onAction, 5 scoped CSS + enum→fixed-class); gen:skill freshness gate
(embedded catalog JSON + SKILL block match `defineCatalog`).

Host (`cargo test -p lb-host --test dashboard_genui_test`) — 8 tests:
```
test accepts_a_well_formed_genui_cell ... ok
test rejects_unknown_component ... ok       test rejects_absent_or_bad_version ... ok
test rejects_dangling_root ... ok           test rejects_missing_options_genui ... ok
test rejects_oversized_spec ... ok          test deny_without_save_cap ... ok           (capability-DENY)
test workspace_isolation_of_a_genui_dashboard ... ok                                     (workspace-ISOLATION)
 test result: ok. 8 passed; 0 failed
```
Dashboard round-trip suite still green (`dashboard_test`: 10 passed). Node binary builds (skill
embeds). `cargo fmt --check` clean.

UI unit (`pnpm test` scope) — genui data helpers + dashboard suite:
```
✓ src/features/dashboard/views/genui/genuiData.test.ts (9)   ← empty-source v3 trap, refId shaping, deny
 Test Files  17 passed (17)   Tests  133 passed (133)   (whole dashboard suite, incl. cellEditorState round-trip)
```

Gateway integration (`pnpm test:gateway src/features/dashboard/views/genui/genui.gateway.test.tsx`) —
real spawned node, 4 tests:
```
✓ saves a well-formed genui cell, reloads it, and RENDERS without the adapter
✓ REJECTS a malformed genui cell at save (Decision 6 — headless-author loud rejection)
✓ round-trips a genui cell with the empty-source v3 trap intact (binding not broken)
✓ DENIES a save without the dashboard.save cap
 Test Files  1 passed (1)   Tests  4 passed (4)
```
(The full `test:gateway` suite has a PRE-EXISTING baseline of 36 failed files / 114 failed tests when
run all-at-once against the one shared serial node — verified IDENTICAL on the pristine tree via
`git stash`; my changes add zero new failures. Known shared-node timing flakiness, per
[[flaky-bus-timing-tests]]. My genui gateway file passes reliably in isolation.)

The one bug hit + fixed this session: the `TargetProbe` setState-in-render warning —
`docs/debugging/genui/genui-probe-setstate-in-render.md` (regression: the gateway test's
`console.error` spy).

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
