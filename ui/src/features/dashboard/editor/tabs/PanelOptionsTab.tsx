// The Panel options tab (viz panel-editor scope) — the per-VIZ options (how the chart draws as a
// whole). For `timeseries` that is legend (show/displayMode/placement/calcs) + tooltip (mode/sort),
// names verbatim from Grafana's `VizLegendOptions`/`VizTooltipOptions` (chart-types scope). Other views
// (Phase 2) plug their own option editor in here keyed by `state.view`. The per-FIELD options
// (unit/decimals/draw-style) are the Field tab's job, not this one (Grafana's line). One responsibility:
// edit the per-viz options.

import { Button } from "@/components/ui/button";
import type { EditorState } from "../cellEditorState";
import {
  readTimeseriesOptions,
  type LegendDisplayMode,
  type LegendPlacement,
  type TooltipDisplayMode,
} from "../../views/timeseries/options";

const FIELD =
  "h-8 rounded-md border border-border bg-bg px-2.5 text-xs text-fg focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/20";

interface Props {
  state: EditorState;
  patch: (next: Partial<EditorState>) => void;
}

const LEGEND_CALCS = ["lastNotNull", "last", "mean", "max", "min", "sum"];

export function PanelOptionsTab({ state, patch }: Props) {
  if (state.view !== "timeseries") {
    return (
      <div className="py-3 text-xs text-muted" aria-label="panel options tab">
        Per-viz options for {state.view} arrive with that panel type (Phase 2).
      </div>
    );
  }
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
      <label className="flex items-center gap-2">
        {/* eslint-disable-next-line no-restricted-syntax -- native checkbox; no shadcn Checkbox primitive (dashboard.md follow-up) */}
        <input
          type="checkbox"
          aria-label="legend show"
          checked={opts.legend.showLegend}
          onChange={(e) => setLegend({ showLegend: e.target.checked })}
        />
        Show legend
      </label>
      <label className="grid gap-1 text-muted">
        Display mode
        {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive yet (dashboard.md follow-up) */}
        <select
          aria-label="legend displayMode"
          className={FIELD}
          value={opts.legend.displayMode}
          onChange={(e) => setLegend({ displayMode: e.target.value as LegendDisplayMode })}
        >
          <option value="list">list</option>
          <option value="table">table</option>
          <option value="hidden">hidden</option>
        </select>
      </label>
      <label className="grid gap-1 text-muted">
        Placement
        {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive yet (dashboard.md follow-up) */}
        <select
          aria-label="legend placement"
          className={FIELD}
          value={opts.legend.placement}
          onChange={(e) => setLegend({ placement: e.target.value as LegendPlacement })}
        >
          <option value="bottom">bottom</option>
          <option value="right">right</option>
        </select>
      </label>
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
      <label className="grid gap-1 text-muted">
        Mode
        {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive yet (dashboard.md follow-up) */}
        <select
          aria-label="tooltip mode"
          className={FIELD}
          value={opts.tooltip.mode}
          onChange={(e) => setTooltip({ mode: e.target.value as TooltipDisplayMode })}
        >
          <option value="single">single</option>
          <option value="multi">multi</option>
          <option value="none">none</option>
        </select>
      </label>
    </div>
  );
}
