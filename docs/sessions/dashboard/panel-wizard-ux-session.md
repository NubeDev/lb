# Session: panel-wizard UX pass (chart-type step + preview)

**Date:** 2026-07-08 · **Area:** `ui/src/features/panel-builder/wizard/` · **Status:** shipped, green

## The ask

The new-panel wizard's chart-type step used its space poorly and lacked parity with the panel editor
(Data Studio):

1. Picking **Template** / **AI widget** left the wizard with no authoring surface — the editor's
   render-template CodeMirror editor and "Copy AI prompt" never mounted.
2. The Plot section embedded PlotBuilder's own mini-preview — a duplicate chart beside the wizard's
   pinned preview.
3. The step ↔ preview split was fixed; users wanted to drag it.
4. The Chart type step should hold the *basic*, visual settings; step 3 (Options) is the *advanced* set.
5. The preview needed a way to view the underlying data as a table or JSON.

## What shipped

- `charts/PlotBuilder.tsx` — new `preview?: boolean` prop (default `true`, existing hosts unchanged).
  `preview={false}` renders controls only, full-width (chart-type radios go 6-up); threaded through
  `tabs/PlotAxesTab.tsx`.
- `wizard/ChartTypeStep.tsx` — reframed as "pick a type + basic setup". Plottable views mount the SAME
  `PlotAxesTab` with `preview={false}` (the pinned preview is the one chart); `template` mounts the
  editor's `TemplateOptionsEditor` (CodeMirror body + `CopyTemplatePrompt`); `genui` mounts
  `GenUiAuthorTab`. No wizard-only authoring surface — reuse, no drift.
- `wizard/WizardPreview.tsx` (new) — the pinned preview with a display-only **Chart | Table | JSON**
  toggle. Chart = `PreviewPane`/`OptionFocusPreview` (unchanged paths); Table = PreviewPane's existing
  `tableView` override; JSON pretty-prints the draft's resolved rows with a copy button. Never touches
  the saved cell.
- `wizard/useSplitPane.ts` (new) — draggable + keyboard-resizable separator between step and preview;
  fraction clamped 0.28–0.72, rounded to 3 decimals, persisted in `localStorage`
  (`lb.panel-wizard.split`).
- `wizard/PanelWizard.tsx` — resolves the draft's rows once via `usePanelData` (useVizQuery's fetch key
  dedupes — no second query) and provides `ResultRowsProvider`, so the template step's "Copy AI prompt"
  embeds the draft's REAL rows, same as the editor. Mounts the split + `WizardPreview`; passes `ws`
  down for the genui author tab.
- `wizard/OptionsStep.tsx` — header copy reframed as "Advanced options" (the basics live on the
  chart-type step).

Rejected alternative: a second, wizard-local template editor / data grid — banned by the no-drift rule;
everything reuses the editor's shipped surfaces.

## Tests (green)

- New `wizard/wizardPreviewModes.gateway.test.tsx` (real gateway, real seeded rows — rule 9):
  template view mounts the template body editor + Copy AI prompt; JSON preview mode shows the seeded
  value and toggles back to chart; the separator is a keyboard-resizable `role="separator"`.
- `pnpm test:gateway src/features/panel-builder/wizard` — 6 files, 18 tests passed.
- `pnpm vitest run src/features/panel-builder src/features/charts` — 16 files, 90 tests passed.
- `tsc --noEmit` clean.

## Follow-up in the same session: the pie legend wall

Feeding a timeseries to a pie drew ~100 invisible timestamp slivers and the legend swallowed the whole
preview. Three fixes:

- `lib/charts/pieSlices.ts` (new) — `capPieSlices`: merge duplicate names (sum), keep the top
  `MAX_PIE_SLICES`(12) by value, bucket the tail into an explicit "Other (n)" slice (aggregated, never
  hidden). Used by BOTH pie renderers: `buildPlot.aggregate` (the plot path) and `PieChartPanel`
  (the piechart view). Unit-tested (`pieSlices.test.ts`).
- `widgets/recharts.tsx` `PieChartSvg` — was a fixed 280×180 viewport stretched by CSS (the SAME bug the
  sparkline comment documents): the legend's internal height swallowed the plot area, so the pie never
  drew. Now `ResponsiveContainer` + a contained scrolling legend (`maxHeight: 72`), matching PlotChart.
- `charts/PlotBuilder.tsx` — an honest hint when a pie's category is a `time` field ("every timestamp
  becomes a slice — pick a categorical field").

## Debugging notes

- One test-only float issue: `0.48 - 0.02*2` → `0.4399…` broke a style assertion. Fixed in product code
  by rounding the split fraction to 3 decimals in `useSplitPane.clamp` (stable style strings). No
  separate debugging entry — caught before commit by the new regression test.
