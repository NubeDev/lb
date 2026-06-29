// The `gauge` per-viz options editor (viz panel-editor scope). Edits the typed `GaugeOptions` —
// reduceOptions + threshold labels/markers + orientation. One responsibility: edit a gauge cell's `options`.

import type { EditorState } from "../../cellEditorState";
import { readGaugeOptions, type VizOrientation } from "../../../views/gauge/options";
import { SelectField, ToggleField } from "./controls";
import { ReduceOptionsEditor } from "./ReduceOptionsEditor";

export function GaugeOptionsEditor({ state, patch }: { state: EditorState; patch: (n: Partial<EditorState>) => void }) {
  const o = readGaugeOptions(state.options);
  const set = (next: Partial<typeof o>) => patch({ options: { ...state.options, ...next } });

  return (
    <div className="grid gap-3 py-3 text-xs" aria-label="panel options tab">
      <ReduceOptionsEditor value={o.reduceOptions} onChange={(reduceOptions) => set({ reduceOptions })} />
      <ToggleField label="Show threshold markers" checked={o.showThresholdMarkers} onChange={(showThresholdMarkers) => set({ showThresholdMarkers })} />
      <ToggleField label="Show threshold labels" checked={o.showThresholdLabels} onChange={(showThresholdLabels) => set({ showThresholdLabels })} />
      <SelectField<VizOrientation>
        label="Orientation"
        value={o.orientation}
        options={["auto", "horizontal", "vertical"]}
        onChange={(orientation) => set({ orientation })}
      />
    </div>
  );
}
