// The `bargauge` per-viz options editor (viz panel-editor scope). Edits the typed `BarGaugeOptions` —
// reduceOptions + display/value mode + orientation + showUnfilled. One responsibility: edit a bargauge
// cell's `options`.

import type { EditorState } from "../../cellEditorState";
import {
  readBarGaugeOptions,
  type BarGaugeDisplayMode,
  type BarGaugeValueMode,
  type VizOrientation,
} from "../../../views/bargauge/options";
import { SelectField, ToggleField } from "./controls";
import { ReduceOptionsEditor } from "./ReduceOptionsEditor";

export function BarGaugeOptionsEditor({ state, patch }: { state: EditorState; patch: (n: Partial<EditorState>) => void }) {
  const o = readBarGaugeOptions(state.options);
  const set = (next: Partial<typeof o>) => patch({ options: { ...state.options, ...next } });

  return (
    <div className="grid gap-3 py-3 text-xs" aria-label="panel options tab">
      <ReduceOptionsEditor value={o.reduceOptions} onChange={(reduceOptions) => set({ reduceOptions })} />
      <SelectField<BarGaugeDisplayMode>
        label="Display mode"
        value={o.displayMode}
        options={["gradient", "basic", "lcd"]}
        onChange={(displayMode) => set({ displayMode })}
      />
      <SelectField<BarGaugeValueMode>
        label="Value mode"
        value={o.valueMode}
        options={["color", "text", "hidden"]}
        onChange={(valueMode) => set({ valueMode })}
      />
      <SelectField<VizOrientation>
        label="Orientation"
        value={o.orientation}
        options={["auto", "horizontal", "vertical"]}
        onChange={(orientation) => set({ orientation })}
      />
      <ToggleField label="Show unfilled" checked={o.showUnfilled} onChange={(showUnfilled) => set({ showUnfilled })} />
    </div>
  );
}
