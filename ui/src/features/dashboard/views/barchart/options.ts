// The `barchart` per-viz options (viz chart-types scope; names + defaults VERBATIM from Grafana's
// `public/app/plugins/panel/barchart/panelcfg.cue` + `common.OptionsWithLegend`/`OptionsWithTooltip`).
// orientation, stacking, showValue, barWidth/groupWidth, xField/colorByField, xTickLabelRotation, plus
// the shared legend/tooltip. Per-FIELD draw (lineWidth/fillOpacity/gradientMode) is `fieldConfig.custom`.
//
// One responsibility: the typed `barchart` options + their Grafana defaults.

import {
  type VizLegendOptions,
  type VizTooltipOptions,
  defaultTimeseriesOptions,
} from "../timeseries/options";
import type { VizOrientation } from "../stat/options";

export type { VizOrientation };
export type StackingMode = "none" | "normal" | "percent";
export type VisibilityMode = "auto" | "always" | "never";

/** Grafana's `barchart` Options (Phase-2 subset). */
export interface BarChartOptions {
  legend: VizLegendOptions;
  tooltip: VizTooltipOptions;
  orientation: VizOrientation;
  stacking: StackingMode;
  showValue: VisibilityMode;
  barWidth: number;
  groupWidth: number;
  xTickLabelRotation: number;
  xField?: string;
  colorByField?: string;
}

/** Grafana defaults (panelcfg `*`): orientation auto, stacking none, showValue auto, barWidth 0.97,
 *  groupWidth 0.7, xTickLabelRotation 0; the shared legend/tooltip defaults. */
export function defaultBarChartOptions(): BarChartOptions {
  const base = defaultTimeseriesOptions();
  return {
    legend: base.legend,
    tooltip: base.tooltip,
    orientation: "auto",
    stacking: "none",
    showValue: "auto",
    barWidth: 0.97,
    groupWidth: 0.7,
    xTickLabelRotation: 0,
  };
}

export function readBarChartOptions(options: Record<string, unknown> | undefined): BarChartOptions {
  const d = defaultBarChartOptions();
  const o = (options ?? {}) as Partial<BarChartOptions>;
  return {
    legend: { ...d.legend, ...(o.legend ?? {}) },
    tooltip: { ...d.tooltip, ...(o.tooltip ?? {}) },
    orientation: o.orientation ?? d.orientation,
    stacking: o.stacking ?? d.stacking,
    showValue: o.showValue ?? d.showValue,
    barWidth: o.barWidth ?? d.barWidth,
    groupWidth: o.groupWidth ?? d.groupWidth,
    xTickLabelRotation: o.xTickLabelRotation ?? d.xTickLabelRotation,
    xField: o.xField,
    colorByField: o.colorByField,
  };
}
