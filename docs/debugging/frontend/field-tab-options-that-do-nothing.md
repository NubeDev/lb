# The Field tab exposes options the renderer never reads (the "overwhelming, half-broken" panel options)

- **Area:** frontend (viz field-config + panel-editor scopes)
- **Status:** open (baseline + regression net landed; the fix is the UX-simplify follow-up)
- **First seen:** 2026-07-07
- **Session:** ../../sessions/frontend/dashboard-field-tab-baseline-session.md
- **Regression test:** ui/src/features/dashboard/views/fieldTabBaseline.gateway.test.tsx

## Symptom (as reported)

> "for a panel/widget we have a section called 'field' and to be honest its overwhelming and complex and
> way to many options, and to be honest i dont even know what works and what doesnt work."

The Field tab (`ui/src/features/panel-builder/tabs/FieldTab.tsx`) is registry-driven: it renders every
`OptionDef` whose `scope === "fieldConfig"` for the current view. The registry was filled out in the
editor-parity phase (steps 2/5) to mirror Grafana's full option surface — but several of those options
have **no render path**: they round-trip cleanly (`registryRoundTrip.test.ts` only checks the editor
de/serializer, not the renderer) yet produce zero visible effect. A user sets them, nothing changes, and
trust in the whole surface erodes. This is the precise source of the "overwhelming and half-broken"
feeling.

## Reproduce

For any DEAD option below: build a cell bound to a real series, set the option through the Field tab's
own write path (`writeOption`), render the panel, and observe the rendered DOM is byte-identical to the
same cell without the option. The baseline test automates this — it is the repro.

## Investigation

Audited every registered `OptionDef` (`options/defs/*`) against every renderer that consumes
`fieldConfig` (`views/*/Panel.tsx`, the shared `fieldconfig/*.ts`, and `widgets/recharts.tsx`). The
classification:

- **LIVE** — the renderer reads the option and the rendered DOM changes observably (text, color,
  `data-draw-style`, arc fraction).
- **DEAD** — the option is stored on the cell and round-trips, but no renderer reads it; setting it
  leaves the rendered output unchanged.

The headline: for **`timeseries`**, ~half the Field-tab options are DEAD. The single-stat family
(`stat`/`gauge`/`bargauge`/`piechart`) is in much better shape — the shared `valueFieldOptions` +
`formatValue` + `applyMappings` bridge covers the standard options faithfully. `table`'s per-column
`custom.*` are all DEAD (the renderer introspects columns but never applies width/align/cell-type/filter).

## Root cause (per option)

The renderer that should read each option simply doesn't. Specifically:

| View | Option | Status | Where it would be read but isn't |
|---|---|---|---|
| timeseries | `mappings` | DEAD | `TimeseriesView.tsx` never calls `applyMappings` (only `StatPanel`/`JsonView` do) |
| timeseries | `links` (data links) | DEAD | no drilldown renderer anywhere |
| timeseries | `custom.lineInterpolation` | DEAD | `recharts.tsx` hardcodes `type="monotone"` |
| timeseries | `custom.gradientMode` | DEAD | read into `custom` bag but never applied to the SVG |
| timeseries | `custom.showPoints` | DEAD | only `drawStyle="points"` shows dots; the per-field toggle is ignored |
| timeseries | `custom.spanNulls` | DEAD | recharts connects gaps regardless |
| timeseries | `custom.axisPlacement` | DEAD | the axis is `<YAxis hide/>` |
| timeseries | `custom.stacking.mode` | DEAD | no stacking in the renderer |
| timeseries | `custom.thresholdsStyle.mode` | DEAD | no threshold line/region rendering |
| table | `custom.width` | DEAD | `TablePanel` renders `<th>/<td>` with no width style |
| table | `custom.align` | DEAD | no alignment class on cells |
| table | `custom.cellOptions.type` | DEAD | no colored-text/background/gauge/sparkline cell renderer |
| table | `custom.filterable` | DEAD | no per-column filter UI |
| timeseries | `color` (scheme) | PARTIAL | only `fixed` mode renders; palette/continuous fall back to accent (`color.ts:41`) |

LIVE options (the regression net confirms these — preserve when simplifying): `displayName`, `unit`,
`decimals`, `min`, `max`, `noValue`, `thresholds` (color), `color` fixed mode, `custom.drawStyle`,
`custom.lineWidth`, `custom.fillOpacity`; and on stat: `mappings`. Per-viz `options.*` (Panel options
tab, not Field tab) are mostly live across all views.

## The deeper cause

The registry was filled to Grafana's full surface intentionally (editor-parity scope goal: "build any
panel without typing JSON"). But the matching **render** work only landed for the standard bridge
(unit/decimals/thresholds/color) + a few graph styles. The remaining options were registered to make the
EDITOR complete, with their render paths as unnamed follow-ups — and unnamed follow-ups are how options
silently go dead. There is no test that an option registered in `defs/*` must affect the renderer, so the
gap is invisible to the green suite.

## Fix

**Phase A (this session) — surface + regression net.** Capture the LIVE/DEAD classification as a real-
gateway test that proves each option's status. This is `fieldTabBaseline.gateway.test.tsx`: it sets each
option through the editor's own `writeOption`, renders the panel against real seeded rows, and asserts a
SPECIFIC observable for LIVE options vs byte-identical output for DEAD ones. The test reads as the
baseline document and is the regression net for the simplify step — when the Field tab is collapsed, a
DEAD option's assertion stays green (intentional drop) and a LIVE option's assertion failing flags an
accidental break.

**Phase B (the follow-up, the user's actual ask) — simplify the UX.** With the baseline as authority,
collapse the Field tab to the options that WORK, present per-view (a timeseries author never sees the
table's dead per-column options; a stat author sees the full standard set). The dead options are either
(a) implemented (the proper fix — wire `applyMappings` into `TimeseriesView`, etc.) or (b) removed from
the registry for the views that don't honor them, so the editor never offers what the renderer won't draw.
Decision per option lives in the session doc + the field-config scope's follow-ups.

## Verification

```
cd ui && npx vitest run --config vitest.gateway.config.ts \
  src/features/dashboard/views/fieldTabBaseline.gateway.test.tsx
# 24 passed (8 standard-LIVE on stat, 3 timeseries-LIVE, 9 timeseries-DEAD, 4 table-DEAD, 1 gauge min/max)
```

## Prevention (the guardrail the class needs)

A registry-level guard: **every `OptionDef` whose `views` includes V must be observed by at least one
test that renders view V and asserts the option changes the output.** `registryRoundTrip.test.ts` already
iterates the whole registry for the round-trip; a sibling test should iterate it for RENDER behavior and
FAIL when a new option is added with no render assertion — so the next option registered to "complete the
editor" cannot go dead silently again. Named as a follow-up in the field-config scope.
