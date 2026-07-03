// The `timeseries` per-viz options editor (viz panel-editor scope) — registry-driven (editor-parity
// step 5): legend show/mode/placement, tooltip mode, stacking, and threshold display style come from
// the option registry via VizOptions; the legend `calcs` (table-mode values) keep their bespoke chip
// row as an extra (a multi-toggle, not a single control). One responsibility: compose the timeseries
// per-viz options.

import { Button } from "@/components/ui/button";
import type { EditorState } from "../../cellEditorState";
import { readTimeseriesOptions } from "../../../views/timeseries/options";
import { VizOptions } from "./VizOptions";

const LEGEND_CALCS = ["lastNotNull", "last", "mean", "max", "min", "sum"];

export function TimeseriesOptionsEditor({ state, patch }: { state: EditorState; patch: (n: Partial<EditorState>) => void }) {
  const opts = readTimeseriesOptions(state.options);
  const toggleCalc = (calc: string) => {
    const has = opts.legend.calcs.includes(calc);
    const calcs = has ? opts.legend.calcs.filter((c) => c !== calc) : [...opts.legend.calcs, calc];
    patch({ options: { ...state.options, legend: { ...opts.legend, calcs } } });
  };
  return (
    <VizOptions
      view="timeseries"
      state={state}
      patch={patch}
      extras={
        <div className="grid gap-1 py-2 text-xs text-muted" data-options-group="legend-values">
          Legend values (table mode)
          <div className="flex flex-wrap gap-1.5">
            {LEGEND_CALCS.map((calc) => (
              <Button
                key={calc}
                variant={opts.legend.calcs.includes(calc) ? "default" : "outline"}
                size="sm"
                aria-label={`legend calc ${calc}`}
                aria-pressed={opts.legend.calcs.includes(calc)}
                className="h-auto px-2 py-0.5 text-[11px]"
                onClick={() => toggleCalc(calc)}
              >
                {calc}
              </Button>
            ))}
          </div>
        </div>
      }
    />
  );
}
