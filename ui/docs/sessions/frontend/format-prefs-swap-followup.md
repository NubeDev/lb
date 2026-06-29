# format.ts → real `format.*` prefs swap — deferred follow-up

Status: DEFERRED (Phase-3 frontend session). `format.ts` is left as-is (the honest fallback).

## Why deferred (the sync→async cascade)

`formatValue(value, opts)` in `ui/src/features/dashboard/fieldconfig/format.ts` is **synchronous**
and is called **during render** from 13 callsites across 8 renderers:

- `views/stat/StatPanel.tsx`, `views/gauge/GaugePanel.tsx`, `views/bargauge/BarGaugePanel.tsx`
- `views/table/TablePanel.tsx`, `views/barchart/BarChartPanel.tsx`, `views/piechart/PieChartPanel.tsx`
- `views/timeseries/TimeseriesView.tsx`, `views/timeseries/Legend.tsx`

The real prefs formatter (`format.quantity` / `format.number` / `format.datetime`) is an **async MCP
call** through the widget bridge. Swapping `formatValue` to call it would make `formatValue` return a
`Promise`, which forces every callsite (and each component that renders its result inline — axis ticks,
legend rows, table cells) to become async / suspense-driven. That is far more than a "contained swap";
it is a cascade across the whole render path, so per the task rule we do NOT half-do it.

The fallback stays honest: it shows the canonical value + the static unit label and never invents a
converted value, behind the unchanged `formatValue` signature with `viaPrefs:false`.

## The contained approach (what to do when this is picked up)

Two viable contained designs, in preference order:

1. **format-on-fetch in `viz.query`.** The backend `viz.query` verb already returns canonical `frames`
   + flattened `rows`. Have it (or a thin post-step on the result in `useVizQuery.ts`) attach a
   pre-formatted display string per field, computed against the panel `fieldConfig` + the viewer's
   prefs server-side. Renderers then read the already-formatted string and `formatValue` stays sync
   (or becomes a trivial passthrough). This keeps the single-fetch boundary and avoids any per-cell
   async at render time. **Preferred** — it reuses the one data hook and the one place values flow in.

2. **A prefetched format cache.** A small async hook (`useFormatPrefs`) loads the workspace/viewer
   prefs once and exposes a SYNC `formatValue` closed over the loaded prefs (canonical conversion
   tables in memory). `formatValue` stays sync; only the prefs load is async, done once per panel
   tree, not per value. The `viaPrefs` flag flips to `true` once prefs are loaded.

Either keeps `formatValue`'s signature sync and confines the change. Option 1 is cleanest because the
formatting then lives with the data, not scattered across renderers.

## Files untouched this session

- `ui/src/features/dashboard/fieldconfig/format.ts` — unchanged (fallback retained, `viaPrefs:false`).
