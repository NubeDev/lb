// Per-view default `options` (the per-VIZ option block, Grafana's panelcfg defaults copied verbatim).
// The single owner of "what a fresh <view>'s options look like" — used by `defaultCell` (ADD) and the
// viz-switch in the editor (reset only the per-view options, preserving targets/fieldConfig). Phase 1:
// timeseries. Phase 2: stat/gauge/bargauge/table/barchart/piechart. One responsibility: view → defaults.

import type { View } from "@/lib/dashboard";
import { defaultTimeseriesOptions } from "../views/timeseries/options";
import { defaultStatOptions } from "../views/stat/options";
import { defaultGaugeOptions } from "../views/gauge/options";
import { defaultBarGaugeOptions } from "../views/bargauge/options";
import { defaultBarChartOptions } from "../views/barchart/options";
import { defaultPieChartOptions } from "../views/piechart/options";
import { defaultTableOptions } from "../views/table/options";

/** The Grafana default `options` for `view` (empty for views without a per-viz option block yet). */
export function defaultOptionsForView(view: View): Record<string, unknown> {
  switch (view) {
    case "timeseries":
      return { ...defaultTimeseriesOptions() };
    case "stat":
      return { ...defaultStatOptions() };
    case "gauge":
      return { ...defaultGaugeOptions() };
    case "bargauge":
      return { ...defaultBarGaugeOptions() };
    case "barchart":
      return { ...defaultBarChartOptions() };
    case "piechart":
      return { ...defaultPieChartOptions() };
    case "table":
      return { ...defaultTableOptions() };
    default:
      return {};
  }
}
