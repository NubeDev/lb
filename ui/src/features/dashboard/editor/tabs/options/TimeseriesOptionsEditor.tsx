// The `timeseries` per-viz options editor (viz panel-editor scope) — legend (show/displayMode/placement/
// calcs) + tooltip (mode/sort), names verbatim from Grafana. Extracted from the Phase-1 PanelOptionsTab
// so every view's option editor is one file (FILE-LAYOUT) and PanelOptionsTab is a thin dispatcher. One
// responsibility: edit a timeseries cell's `options`.

import { Button } from "@/components/ui/button";
import type { EditorState } from "../../cellEditorState";
import {
  readTimeseriesOptions,
  type LegendDisplayMode,
  type LegendPlacement,
  type TooltipDisplayMode,
} from "../../../views/timeseries/options";
import { SelectField, ToggleField } from "./controls";

const LEGEND_CALCS = ["lastNotNull", "last", "mean", "max", "min", "sum"];

export function TimeseriesOptionsEditor({ state, patch }: { state: EditorState; patch: (n: Partial<EditorState>) => void }) {
  const opts = readTimeseriesOptions(state.options);
  const setLegend = (next: Partial<typeof opts.legend>) =>
    patch({ options: { ...state.options, legend: { ...opts.legend, ...next } } });
  const setTooltip = (next: Partial<typeof opts.tooltip>) =>
    patch({ options: { ...state.options, tooltip: { ...opts.tooltip, ...next } } });

  const toggleCalc = (calc: string) => {
    const has = opts.legend.calcs.includes(calc);
    setLegend({ calcs: has ? opts.legend.calcs.filter((c) => c !== calc) : [...opts.legend.calcs, calc] });
  };

  return (
    <div className="grid gap-3 py-3 text-xs" aria-label="panel options tab" data-options-group="legend">
      <div className="font-medium text-muted">Legend</div>
      <ToggleField label="Show legend" checked={opts.legend.showLegend} onChange={(showLegend) => setLegend({ showLegend })} />
      <SelectField<LegendDisplayMode>
        label="Display mode"
        value={opts.legend.displayMode}
        options={["list", "table", "hidden"]}
        onChange={(displayMode) => setLegend({ displayMode })}
      />
      <SelectField<LegendPlacement>
        label="Placement"
        value={opts.legend.placement}
        options={["bottom", "right"]}
        onChange={(placement) => setLegend({ placement })}
      />
      <div className="grid gap-1 text-muted">
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

      <div className="mt-2 font-medium text-muted" data-options-group="tooltip">
        Tooltip
      </div>
      <SelectField<TooltipDisplayMode>
        label="Mode"
        value={opts.tooltip.mode}
        options={["single", "multi", "none"]}
        onChange={(mode) => setTooltip({ mode })}
      />
    </div>
  );
}
