# Chart-type picker — add per-view icons

## Ask

The wizard's "2. Chart type" step (`VizPicker`) rendered its buttons as text-only
labels ("Time series", "Bar chart", "Stat", …). User asked for an icon per chart/
panel type so the row scans by shape, not just by reading every label.

## What changed

`ui/src/features/panel-builder/VizPicker.tsx`:
- `VIEWS`, `FLOW_CONTROL_VIEWS`, `FLOW_READ_VIEWS` each gained an `icon: LucideIcon`
  field, mirroring the icon choices `VizGallery.tsx` already ships for its
  non-thumbnail cards (`table`→Table2, `genui`→Sparkles, `template`→LayoutTemplate,
  `insights`→Lightbulb) plus new ones for the chart-shaped views that `VizGallery`
  renders as live thumbnails instead (`timeseries`→LineChart, `barchart`→BarChart3,
  `stat`→Hash, `gauge`→Gauge, `bargauge`→BarChartHorizontal, `piechart`→PieChart)
  and the flow-control/read views (`switch`→ToggleLeft, `slider`→SlidersHorizontal,
  `json`→Braces, `jsonview`→FileJson).
- Both button-render sites (`FLOW_CONTROL_VIEWS` early-return branch and the
  standard/flow-output branch) now render `<v.icon size={14} />` before the label.
  The shared `Button` base class already carries `inline-flex items-center gap-2`
  and `[&_svg]:shrink-0`, so no new layout code was needed.

## Why not touch `VizGallery.tsx`

`VizGallery` already has icons for its 4 labeled cards; its 6 chart-shaped cards
render LIVE thumbnails (real `WidgetView` mini-renders), not icons, by design — a
richer picker used once data exists. `VizPicker` is the plain button row (used in
the wizard step 2 before/without a gallery), which had no icons at all. This session
only touched `VizPicker`.

## Verification

- `npx tsc --noEmit` clean.
- `vitest run --config vitest.gateway.config.ts panelWizard.gateway.test.tsx` — 4/4
  pass. Tests select buttons via `aria-label="viz <id>"`, unaffected by the added
  icon child.
- `vitest run VizGallery.test.tsx` — 6/6 pass (untouched file, sanity check only).
- Rendered a static reproduction of the button row (real sizing: 14px icon, 8px gap,
  `h-auto px-2.5 py-1`, dark theme colors) via Playwright to confirm no wrapping/
  crowding regressions before shipping.
