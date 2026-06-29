// The timeseries legend (viz chart-types scope, the `timeseries` `legend` option). Renders the series
// name + (in `table` display mode) the reducer calcs the legend declares (`mean`/`max`/`last`/…), each
// value formatted through the ONE user-prefs bridge (`fieldconfig/format.ts`) — never a local number
// format. Phase 1 charts a single field, so the legend has one row; the list/table structure is
// Grafana's so adding fields later is additive. One responsibility: the legend markup + its calcs.

import type { FieldOptions } from "@/lib/dashboard";
import type { VizLegendOptions } from "./options";
import { formatValue } from "../../fieldconfig/format";

/** A reducer calc over the field's points (the legend's `calcs`). Pure; formatting happens after. */
function reduceCalc(calc: string, points: number[]): number | null {
  if (points.length === 0) return null;
  switch (calc) {
    case "mean":
      return points.reduce((a, b) => a + b, 0) / points.length;
    case "max":
      return Math.max(...points);
    case "min":
      return Math.min(...points);
    case "sum":
      return points.reduce((a, b) => a + b, 0);
    case "first":
      return points[0];
    case "last":
    case "lastNotNull":
      return points[points.length - 1];
    default:
      return null;
  }
}

interface Props {
  legend: VizLegendOptions;
  seriesName: string;
  color: string;
  points: number[];
  opts: FieldOptions;
}

export function TimeseriesLegend({ legend, seriesName, color, points, opts }: Props) {
  if (!legend.showLegend || legend.displayMode === "hidden") return null;
  const isTable = legend.displayMode === "table";

  return (
    <div
      className={`mt-1 flex gap-3 text-[11px] text-muted ${legend.placement === "right" ? "flex-col" : "flex-wrap"}`}
      aria-label="timeseries legend"
    >
      <span className="inline-flex items-center gap-1.5">
        <span className="inline-block h-2 w-2 rounded-full" style={{ background: color }} aria-hidden />
        <span className="text-fg">{seriesName}</span>
      </span>
      {isTable &&
        legend.calcs.map((calc) => {
          const v = reduceCalc(calc, points);
          return (
            <span key={calc} className="tabular-nums" aria-label={`legend ${calc}`}>
              {calc}: {v == null ? "—" : formatValue(v, opts).text}
            </span>
          );
        })}
    </div>
  );
}
