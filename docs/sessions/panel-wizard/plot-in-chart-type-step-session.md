# Session: Plot editor in the wizard's chart-type step

Date: 2026-07-08 · Area: `ui/src/features/panel-builder/wizard`

## Ask

The Data Studio editor's **Plot** section (`PlotAxesTab` → the shared `PlotBuilder`,
"Pick the chart type and assign the fields to the X and Y axes…") produces better
charts than the bare view picker. The new-panel wizard's step 2 (Chart type) only
mounted `VizPicker`. Ask: surface the Plot options on step 2.

## What changed

- `wizard/ChartTypeStep.tsx` — below the shipped `VizPicker`, when the chosen view is
  plottable (`PLOTTABLE_VIEWS`: timeseries/barchart/piechart), mount the editor's SAME
  `PlotAxesTab` under a "Plot" heading. New props: `draft` (the wizard's serialized
  preview cell — supplies the live query fields), `patch` (writes `options.plot`
  without a view reset), `refreshKey`.
- `wizard/PanelWizard.tsx` — passes `cell` / `patch` / `refreshKey` from
  `useWizardPreview` into `ChartTypeStep`.

**No second chart surface** (the load-bearing wizard rule): the plot editor is the
editor's own component; the spec persists as `options.plot` through the same
`editorStateToCell` save path. Switching views still resets per-view options
(`withViewReset`), mirroring the editor — a plot built for timeseries doesn't leak
into stat.

Rejected alternative: a wizard-only PlotBuilder mount — would duplicate the
field-inference/empty/denied states `PlotAxesTab` already handles.

## Tests

`pnpm vitest run --config vitest.gateway.config.ts src/features/panel-builder/wizard`
→ **15/15 green** (real gateway, seeded rows). New regression in
`panelWizard.gateway.test.tsx`: chart-type step shows `plot axes tab` for the default
timeseries view and removes it after switching to stat. `tsc --noEmit` clean.

## Open

- The wizard now shows the PlotBuilder's inline preview *and* the pinned PreviewPane
  on step 2 (two charts on one screen). Acceptable for now; if noisy, collapse the
  PlotBuilder preview in wizard context.
- User suspects the non-plot chart set (legacy per-view renderers) looks worse than
  `PlotChart`; converging more views onto the plot pipeline is a possible follow-up.
