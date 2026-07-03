// The `barchart` per-viz options editor (viz panel-editor scope). Edits the typed `BarChartOptions` —
// orientation + stacking + showValue + legend show + tooltip mode. One responsibility: edit a barchart
// cell's `options`.

import type { EditorState } from "@/lib/panel-kit/cellEditorState";
import {
  readBarChartOptions,
  type VizOrientation,
  type StackingMode,
  type VisibilityMode,
} from "@/features/dashboard/views/barchart/options";
import type { TooltipDisplayMode } from "@/features/dashboard/views/timeseries/options";
import { SelectField, ToggleField } from "./controls";

export function BarChartOptionsEditor({ state, patch }: { state: EditorState; patch: (n: Partial<EditorState>) => void }) {
  const o = readBarChartOptions(state.options);
  const set = (next: Partial<typeof o>) => patch({ options: { ...state.options, ...next } });

  return (
    <div className="grid gap-3 py-3 text-xs" aria-label="panel options tab">
      <SelectField<VizOrientation>
        label="Orientation"
        value={o.orientation}
        options={["auto", "horizontal", "vertical"]}
        onChange={(orientation) => set({ orientation })}
      />
      <SelectField<StackingMode>
        label="Stacking"
        value={o.stacking}
        options={["none", "normal", "percent"]}
        onChange={(stacking) => set({ stacking })}
      />
      <SelectField<VisibilityMode>
        label="Show value"
        value={o.showValue}
        options={["auto", "always", "never"]}
        onChange={(showValue) => set({ showValue })}
      />
      <ToggleField label="Show legend" checked={o.legend.showLegend} onChange={(showLegend) => set({ legend: { ...o.legend, showLegend } })} />
      <SelectField<TooltipDisplayMode>
        label="Tooltip"
        value={o.tooltip.mode}
        options={["single", "multi", "none"]}
        onChange={(mode) => set({ tooltip: { ...o.tooltip, mode } })}
      />
    </div>
  );
}
