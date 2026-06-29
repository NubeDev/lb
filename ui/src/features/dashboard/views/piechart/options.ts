// The `piechart` per-viz options (viz chart-types scope; names + defaults VERBATIM from Grafana's
// `public/app/plugins/panel/piechart/panelcfg.cue`). pieType (pie/donut), displayLabels, legend (with
// pie legend `values`), tooltip, the reduceOptions frame→value bridge. Slice values come from
// reduceOptions; slice color/format from `fieldConfig`.
//
// One responsibility: the typed `piechart` options + their Grafana defaults.

import { type ReduceDataOptions, defaultReduceOptions, readReduceOptions } from "../reduce";
import {
  type VizLegendOptions,
  type VizTooltipOptions,
  defaultTimeseriesOptions,
} from "../timeseries/options";

export type PieChartType = "pie" | "donut";
export type PieChartLabel = "name" | "value" | "percent";

/** Grafana's `piechart` Options (Phase-2 subset). `legend` reuses the shared `VizLegendOptions`. */
export interface PieChartOptions {
  reduceOptions: ReduceDataOptions;
  pieType: PieChartType;
  displayLabels: PieChartLabel[];
  legend: VizLegendOptions;
  tooltip: VizTooltipOptions;
}

/** Grafana defaults: pieType "pie", no display labels, the shared legend/tooltip defaults. (Grafana's
 *  piechart legend default shows the list; we reuse the timeseries legend/tooltip defaults verbatim.) */
export function defaultPieChartOptions(): PieChartOptions {
  const base = defaultTimeseriesOptions();
  return {
    reduceOptions: defaultReduceOptions(),
    pieType: "pie",
    displayLabels: [],
    legend: base.legend,
    tooltip: base.tooltip,
  };
}

export function readPieChartOptions(options: Record<string, unknown> | undefined): PieChartOptions {
  const d = defaultPieChartOptions();
  const o = (options ?? {}) as Partial<PieChartOptions>;
  return {
    reduceOptions: readReduceOptions(o.reduceOptions),
    pieType: o.pieType ?? d.pieType,
    displayLabels: Array.isArray(o.displayLabels) ? o.displayLabels : d.displayLabels,
    legend: { ...d.legend, ...(o.legend ?? {}) },
    tooltip: { ...d.tooltip, ...(o.tooltip ?? {}) },
  };
}
