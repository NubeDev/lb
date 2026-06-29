// Per-view default `options` (the per-VIZ option block, Grafana's panelcfg defaults copied verbatim).
// The single owner of "what a fresh <view>'s options look like" — used by `defaultCell` (ADD) and the
// viz-switch in the editor (reset only the per-view options, preserving targets/fieldConfig). Phase 1:
// timeseries. Phase 2 views add their entry here. One responsibility: view → default options.

import type { View } from "@/lib/dashboard";
import { defaultTimeseriesOptions } from "../views/timeseries/options";

/** The Grafana default `options` for `view` (empty for views without a per-viz option block yet). */
export function defaultOptionsForView(view: View): Record<string, unknown> {
  switch (view) {
    case "timeseries":
      return { ...defaultTimeseriesOptions() };
    default:
      return {};
  }
}
