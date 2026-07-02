// Bridge the host-computed `ChartSpec` (the query worker's auto-pick) onto the shared `PlotSpec` the
// one renderer/builder understands. The host still picks the DEFAULT chart (so every subscriber opens on
// the same view); this just re-expresses it in the shared model so the in-channel result renders through
// the exact same `PlotChart` the dashboard uses — and so "Customize" starts from the host's pick.
//
// One responsibility: ChartSpec ↔ PlotSpec for the channel surface.

import type { PlotSpec } from "@/lib/charts";
import type { ChartSpec } from "@/lib/channel/payload.types";

/** The host `ChartSpec` as a shared `PlotSpec`. A histogram bins the `x` field; line/bar plot the series
 *  columns against `x`. */
export function chartSpecToPlotSpec(chart: ChartSpec): PlotSpec {
  if (chart.type === "histogram") {
    return { type: "histogram", xField: "", yFields: [chart.x], bins: chart.bins };
  }
  return {
    type: chart.type === "bar" ? "bar" : "line",
    xField: chart.x,
    yFields: chart.series.map((s) => s.field),
    smooth: chart.type !== "bar",
  };
}
