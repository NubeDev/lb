// The per-(view, option) LIVE/DEAD table for the panel wizard (panel-wizard scope, step 1 — the
// "declare + test" house pattern). The wizard's headline is preview-per-option: each option card shows
// a live mini-preview of its effect, AND dead options surface themselves honestly ("no visible effect —
// renderer pending"). That honest note reads THIS table, not a hand-maintained freehand classification.
//
// Sourced from `fieldTabBaseline.gateway.test.tsx`'s audit: every registered Field/option is classified
// LIVE (setting it observably changes the rendered DOM) or DEAD (stored + round-tripped, zero visible
// effect). The baseline test ENFORCES this table (a) exhaustively — every option the registry exposes
// for a wizard view has a row here — and (b) accurately — every row the baseline proves DEAD via a real
// render is declared DEAD here. Declare + test; never declare-only. Mirrors the radius-scale guard and
// the registry round-trip test.
//
// One responsibility: the per-(view, option) liveness declaration + lookup.

import type { View } from "@/lib/dashboard";
import type { OptionDef } from "./types";

/** The views the wizard authors. Scope of `OPTION_LIVENESS`: every `(view, optionId)` pair where the
 *  registry's `appliesToView(view)` returns true for one of these views must have a row. A view outside
 *  this set (e.g. `plot`, `d3`, `ext:…`) is not wizard-authored — it has no liveness entry by design. */
export const WIZARD_VIEWS: View[] = ["timeseries", "barchart", "stat", "gauge", "bargauge", "piechart", "table"];

/**
 * The declared liveness table. Outer key = view; inner key = option id; value = `live`.
 *
 * Updating: when a renderer starts honoring a previously-dead option, flip its row to `true` here AND
 * the baseline test's DEAD list loses it (the test guides you). When an option is ADDED to the registry,
 * the exhaustiveness test fails until a row appears here — forcing an explicit liveness call.
 */
export const OPTION_LIVENESS: Record<string, Record<string, boolean>> = {
  timeseries: {
    // --- standard (universal) ---
    displayName: true,
    unit: true,
    decimals: true,
    min: true,
    max: true,
    noValue: true,
    color: true, // PARTIAL today (only `fixed` mode renders); treated as LIVE — the wizard surfaces the rest
    thresholds: true, // colors the line/value
    mappings: false, // TimeseriesView never calls applyMappings — DEAD (proven)
    links: false, // no drilldown renderer anywhere — DEAD (proven)
    // --- graph styles (custom.*) ---
    "custom.drawStyle": true,
    "custom.lineInterpolation": false, // recharts always uses monotone — DEAD
    "custom.lineWidth": true,
    "custom.fillOpacity": true,
    "custom.gradientMode": false, // read into `custom` bag, never applied — DEAD
    "custom.showPoints": false, // only drawStyle="points" shows dots; the per-field toggle is ignored — DEAD
    "custom.spanNulls": false, // recharts connects gaps regardless — DEAD
    "custom.axisPlacement": false, // the axis is `<YAxis hide/>` — DEAD
    // --- per-viz options.* ---
    "legend.showLegend": true,
    "legend.displayMode": true,
    "legend.placement": true,
    "tooltip.mode": true,
    "custom.stacking.mode": false, // no stacking in the renderer — DEAD
    "custom.thresholdsStyle.mode": false, // no threshold line/region rendering — DEAD
  },
  barchart: {
    // Only the universal standard options register for barchart (no per-viz defs in the registry yet).
    // The renderer rides the shared valueFieldOptions/formatValue/categoryColor bridge — text + color
    // options are LIVE; it never calls applyMappings and its empty state is a hardcoded "no data yet"
    // (opts.noValue is never read), so those are DEAD.
    displayName: true,
    unit: true,
    decimals: true,
    min: true, // threshold bounds via categoryColor → thresholdColor
    max: true,
    noValue: false, // BarChartPanel renders "no data yet" regardless — DEAD (proven)
    color: true,
    thresholds: true,
    mappings: false, // BarChartPanel never calls applyMappings — DEAD (proven)
    links: false, // no drilldown renderer anywhere — DEAD (proven)
  },
  stat: {
    // --- standard (universal) ---
    displayName: true,
    unit: true,
    decimals: true,
    min: true,
    max: true,
    noValue: true,
    color: true,
    thresholds: true,
    mappings: true, // the faithful renderer — StatPanel honors applyMappings
    links: false, // no drilldown renderer anywhere — DEAD
    // --- per-viz options.* ---
    colorMode: true,
    graphMode: true,
    textMode: true,
    justifyMode: true,
    showPercentChange: true,
    orientation: true,
  },
  gauge: {
    displayName: true,
    unit: true,
    decimals: true,
    min: true, // rescales the arc — LIVE
    max: true, // rescales the arc — LIVE
    noValue: true,
    color: true,
    thresholds: true,
    mappings: true,
    links: false,
    showThresholdLabels: true,
    showThresholdMarkers: true,
    orientation: true,
  },
  bargauge: {
    displayName: true,
    unit: true,
    decimals: true,
    min: true,
    max: true,
    noValue: true,
    color: true,
    thresholds: true,
    mappings: true,
    links: false,
    displayMode: true,
    showUnfilled: true,
    orientation: true,
  },
  piechart: {
    displayName: true,
    unit: true,
    decimals: true,
    min: true,
    max: true,
    noValue: true,
    color: true,
    thresholds: true,
    mappings: true,
    links: false,
    pieType: true,
  },
  table: {
    // --- standard (universal) ---
    displayName: true,
    unit: true,
    decimals: true,
    min: true,
    max: true,
    noValue: true,
    color: true,
    thresholds: true,
    mappings: false, // TablePanel never calls applyMappings — DEAD (same root cause as timeseries)
    links: false,
    // --- per-viz options.* ---
    showHeader: true,
    cellHeight: true,
    enablePagination: true,
    "footer.show": true,
    "footer.reducer": true,
    // --- per-column custom.* (Grafana's TableFieldOptions) — none honored by TablePanel ---
    "custom.width": false,
    "custom.align": false,
    "custom.cellOptions.type": false,
    "custom.filterable": false,
  },
};

/** The dead options for `view` (the wizard's "renderer pending" annotations). Empty for an unknown view. */
export function deadOptionsForView(view: View): string[] {
  const rows = OPTION_LIVENESS[view];
  if (!rows) return [];
  return Object.keys(rows).filter((id) => rows[id] === false);
}

/** Is `optionId` LIVE for `view`? Throws if the pair has no declared row — the wizard never silently
 *  falls back to "live" for an unclassified option (an unclassified option is a finding, not a default). */
export function optionLiveness(view: View, optionId: string): boolean {
  const rows = OPTION_LIVENESS[view];
  if (!rows || !(optionId in rows)) {
    throw new Error(`optionLiveness: no row for ${view}/${optionId} — add it (declare + test)`);
  }
  return rows[optionId];
}

/** Non-throwing variant for code paths that prefer a default (e.g. a sanity check before render). */
export function isLive(view: View, optionId: string): boolean {
  try {
    return optionLiveness(view, optionId);
  } catch {
    return false;
  }
}

/** Type narrowing helper for tests: every option in `defs` for `view` has a declared liveness row. */
export function hasRowForView(view: View, defs: OptionDef[]): boolean {
  const rows = OPTION_LIVENESS[view];
  if (!rows) return false;
  return defs.every((d) => d.id in rows);
}
