// The visualization picker (viz panel-editor scope: the viz picker; chart-types scope: the view set +
// "Result-shape ↔ type validation"). Phase 1+2 offer the everyday standard set
// (timeseries/barchart/stat/gauge/bargauge/table/piechart). The picker offers only the views a target's
// data shape can HONESTLY fill (`viewFitsShape`) — a scalar can't be a table, tabular rows can't be a
// gauge — disabling the rest rather than hiding them (honest about why). Switching the viz preserves the
// targets/fieldConfig (panel-editor scope, Risks); only the per-view `options` reset, by the caller.
//
// One responsibility: pick a view, shape-validated.

import { Button } from "@/components/ui/button";
import type { View } from "@/lib/dashboard";
import { type ResultShape, viewFitsShape } from "../views/shape";

/** The buildable standard set (Phase 1+2), in Grafana's panel-picker order. */
const VIEWS: { id: View; label: string }[] = [
  { id: "timeseries", label: "Time series" },
  { id: "barchart", label: "Bar chart" },
  { id: "stat", label: "Stat" },
  { id: "gauge", label: "Gauge" },
  { id: "bargauge", label: "Bar gauge" },
  { id: "piechart", label: "Pie chart" },
  { id: "table", label: "Table" },
];

interface Props {
  view: View;
  onChange: (view: View) => void;
  /** The detected shape of the current target's data — disables views that shape can't honestly fill.
   *  `unknown` (no data yet) leaves every view enabled. */
  shape?: ResultShape;
}

export function VizPicker({ view, onChange, shape = "unknown" }: Props) {
  return (
    <div className="grid gap-2" aria-label="visualization picker">
      <div className="flex flex-wrap gap-1.5">
        {VIEWS.map((v) => {
          const fits = viewFitsShape(v.id, shape);
          const selected = view === v.id;
          return (
            <Button
              key={v.id}
              variant={selected ? "default" : "outline"}
              size="sm"
              disabled={!fits && !selected}
              aria-label={`viz ${v.id}`}
              aria-pressed={selected}
              aria-disabled={!fits && !selected}
              title={!fits ? `the current data shape can't honestly fill a ${v.id}` : undefined}
              className={`h-auto px-2.5 py-1 ${!fits && !selected ? "cursor-not-allowed border-dashed text-muted/50" : ""}`}
              onClick={() => fits && onChange(v.id)}
            >
              {v.label}
            </Button>
          );
        })}
      </div>
    </div>
  );
}
