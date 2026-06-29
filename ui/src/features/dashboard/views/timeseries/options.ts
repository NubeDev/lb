// The `timeseries` per-viz options shape (viz chart-types scope, "The per-viz options shapes" — names
// taken VERBATIM from Grafana's `common.VizLegendOptions`/`VizTooltipOptions` so import/export is a
// 1:1 map). This is the per-VIZ options (how the chart draws as a whole: legend, tooltip). The
// per-FIELD draw fields (drawStyle/lineWidth/fillOpacity) are `fieldConfig.custom`, owned by the
// field-config layer (see `custom.ts`), not here — Grafana's line, kept exactly.
//
// One responsibility: the typed options + their Grafana defaults. The renderer reads this; nothing
// here renders.

export type LegendDisplayMode = "list" | "table" | "hidden";
export type LegendPlacement = "bottom" | "right";
export type TooltipDisplayMode = "single" | "multi" | "none";
export type SortOrder = "asc" | "desc" | "none";

export interface VizLegendOptions {
  showLegend: boolean;
  displayMode: LegendDisplayMode;
  placement: LegendPlacement;
  /** Reducer calc ids shown per series in `table` legend mode (`mean`/`max`/`last`/…). */
  calcs: string[];
}

export interface VizTooltipOptions {
  mode: TooltipDisplayMode;
  sort: SortOrder;
}

/** The full `timeseries` options (Grafana's `Options` for the panel, Phase-1 subset: legend+tooltip). */
export interface TimeseriesOptions {
  legend: VizLegendOptions;
  tooltip: VizTooltipOptions;
}

/** Grafana's out-of-the-box defaults (chart-types scope, Resolved decision: "Copy Grafana's
 *  panelcfg per-panel option defaults verbatim"). A fresh timeseries panel looks like Grafana's. */
export function defaultTimeseriesOptions(): TimeseriesOptions {
  return {
    legend: { showLegend: true, displayMode: "list", placement: "bottom", calcs: [] },
    tooltip: { mode: "single", sort: "none" },
  };
}

/** Read a cell's `options` as `TimeseriesOptions`, filling any missing field with the Grafana default
 *  (so a v2 cell whose `options` only had a `unit` string still renders with full legend/tooltip
 *  defaults — and the stale `unit` string is ignored here; units live in `fieldConfig` now). */
export function readTimeseriesOptions(options: Record<string, unknown> | undefined): TimeseriesOptions {
  const d = defaultTimeseriesOptions();
  const o = (options ?? {}) as Partial<TimeseriesOptions>;
  return {
    legend: { ...d.legend, ...(o.legend ?? {}) },
    tooltip: { ...d.tooltip, ...(o.tooltip ?? {}) },
  };
}
