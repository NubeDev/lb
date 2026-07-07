# Session — Field-tab baseline audit (what works vs what doesn't)

**Date:** 2026-07-07
**Scope:** [`scope/frontend/dashboard/viz/field-config-scope.md`](../../scope/frontend/dashboard/viz/field-config-scope.md)
+ [`panel-editor-scope.md`](../../scope/frontend/dashboard/viz/panel-editor-scope.md)
**Status:** baseline + regression net landed. The UX-simplify step is the named follow-up.
**Debug entry:** [`debugging/frontend/field-tab-options-that-do-nothing.md`](../../debugging/frontend/field-tab-options-that-do-nothing.md)

## The ask

The user reported the panel editor's **Field** section is overwhelming, has too many options, and they
don't know which ones actually work. The explicit goal of this session: **get a baseline of what works
and what doesn't** before touching the UX. Tests were to be the proof; gateway render tests chosen over
playwright screenshots (faster, more precise, matches project doctrine).

## What I read first

- The viz docs (`scope/frontend/dashboard/viz/`) — `field-config-scope.md` (the option taxonomy),
  `panel-editor-scope.md` (the Field tab UX), `README.md` (phasing: editor-parity S8 shipped the full
  registry).
- The registry + render split:
  - **Editor side** — `ui/src/features/panel-builder/options/` (`registry.ts`, `OptionGroups.tsx`,
    `Control.tsx`, `binding.ts`, `defs/{standard,timeseriesGraph,timeseriesViz,table,singleStat}.ts`).
    The Field tab (`tabs/FieldTab.tsx`) renders every `OptionDef` with `scope==="fieldConfig"` for the
    current view.
  - **Render side** — `ui/src/features/dashboard/fieldconfig/` (`format.ts`, `resolve.ts`,
    `thresholds.ts`, `mappings.ts`, `color.ts`, `units.ts`) + `views/*/{TimeseriesView,StatPanel,...
    }.tsx` + `widgets/recharts.tsx`.

## The headline finding

**~half the Field-tab options for `timeseries` are DEAD** (stored + round-tripped, but never read by the
renderer). `table`'s per-column `custom.*` are all dead. The single-stat family (`stat`/`gauge`/
`bargauge`/`piechart`) is in much better shape — the shared `valueFieldOptions` + `formatValue` +
`applyMappings` bridge covers the standard options faithfully. The full per-option matrix is in the debug
entry; the short of it:

- **timeseries DEAD (9):** `mappings`, `links`, `custom.lineInterpolation`, `custom.gradientMode`,
  `custom.showPoints`, `custom.spanNulls`, `custom.axisPlacement`, `custom.stacking.mode`,
  `custom.thresholdsStyle.mode`.
- **timeseries PARTIAL (1):** `color` scheme — only `fixed` mode renders.
- **table DEAD (4):** `custom.width`, `custom.align`, `custom.cellOptions.type`, `custom.filterable`.
- **LIVE everywhere it matters:** `displayName`, `unit`, `decimals`, `min`, `max`, `noValue`,
  `thresholds`, `custom.drawStyle`, `custom.lineWidth`, `custom.fillOpacity`; + `mappings` on stat.

The registry was filled to Grafana's full surface in the editor-parity phase (its goal: "build any panel
without typing JSON"). The matching render work only landed for the standard bridge + a few graph styles;
the rest were registered to make the EDITOR complete, with their render paths as unnamed follow-ups.
Unnamed follow-ups are how options silently go dead — and `registryRoundTrip.test.ts` only exercises the
de/serializer, not the renderer, so the green suite hid it.

## The deliverable — `fieldTabBaseline.gateway.test.tsx`

`ui/src/features/dashboard/views/fieldTabBaseline.gateway.test.tsx` (24 tests, green). It is the durable
authority behind the matrix above and the regression net for the simplify step. Design:

- **Real gateway, real seeded samples** through the ingest path (rule 9 / testing §0 — no fakes).
- **Each option set through the editor's own `writeOption`** (via `cellToEditorState` → `writeOption` →
  `editorStateToCell`) — byte-identical to a user setting it in the Field tab. So "set option X" in the
  test IS the Field-tab write path.
- **LIVE options** assert a SPECIFIC observable (`unit:"celsius"` → "°C" in the value text; thresholds
  flip the value color to `rgb(220,40,40)`; `drawStyle:"bars"` → `data-draw-style="bars"` on the chart
  host; min/max rescale the gauge arc).
- **DEAD options** assert the rendered DOM is byte-identical to the same cell without the option — i.e.
  setting it had zero visible effect.
- **The recharts gotcha:** recharts generates non-deterministic `clipPath` ids per render, so raw
  `innerHTML` differs even for identical inputs. The comparison strips `<svg>` subtrees (`plainDom`) and
  compares the plain-HTML wrapper — which is also where every Field-tab option's visible effect lands
  (the value readout, the legend, the chart host's `data-draw-style` + `style` color). jsdom's
  `hsl()`→`rgb()` normalization is handled by a `norm()` whitespace-collapse.

When the UX-simplify step collapses the Field tab, this test file is the contract: a DEAD option's
assertion stays green (we intentionally dropped it), a LIVE option's assertion failing flags an
accidental break.

## Green output

```
npx vitest run --config vitest.gateway.config.ts \
  src/features/dashboard/views/fieldTabBaseline.gateway.test.tsx
 ✓ src/features/dashboard/views/fieldTabBaseline.gateway.test.tsx (24 tests) 2104ms
 Test Files  1 passed (1)
      Tests  24 passed (24)
```

## What is deliberately NOT done here

- **No UX changes.** The user's flow is: baseline first → then simplify. This session is the baseline.
- **No dead option is removed or implemented.** Phase B decides per option (implement vs remove from the
  per-view registry) — that is the simplify session's call, now made against a verified matrix.
- **No registry-level exhaustiveness guard yet** (named as a follow-up in the field-config scope): a test
  that FAILS when a new `OptionDef` is registered with no render assertion. The baseline test is
  exhaustive for TODAY's registry; the guard makes it exhaustive for tomorrow's.

## Follow-ups (named, not silent)

1. **The UX-simplify session** — collapse the Field tab to what works, per-view. Input: the matrix in
   the debug entry + the baseline test.
2. **Decide each DEAD option's fate** — implement (e.g. wire `applyMappings` into `TimeseriesView`) or
   remove from the per-view registry. Recorded in the field-config scope's follow-ups.
3. **Registry render-guard** — extend `registryRoundTrip.test.ts`'s exhaustiveness pattern to RENDER
   behavior so a new option can't go dead silently.
4. **`color` palette/continuous modes** — either implement or scope them out of the `ColorSchemeEditor`
   mode list so the picker never offers what the resolver degrades.
