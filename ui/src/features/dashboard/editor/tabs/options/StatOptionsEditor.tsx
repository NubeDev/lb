// The `stat` per-viz options editor (viz panel-editor scope). Edits the typed `StatOptions` —
// reduceOptions (the frame→value bridge) + graph/color/justify/text mode + orientation. Names verbatim
// from Grafana. One responsibility: edit a stat cell's `options`.

import type { EditorState } from "../../cellEditorState";
import {
  readStatOptions,
  type BigValueGraphMode,
  type BigValueColorMode,
  type BigValueTextMode,
  type VizOrientation,
} from "../../../views/stat/options";
import { SelectField } from "./controls";
import { ReduceOptionsEditor } from "./ReduceOptionsEditor";

export function StatOptionsEditor({ state, patch }: { state: EditorState; patch: (n: Partial<EditorState>) => void }) {
  const o = readStatOptions(state.options);
  const set = (next: Partial<typeof o>) => patch({ options: { ...state.options, ...next } });

  return (
    <div className="grid gap-3 py-3 text-xs" aria-label="panel options tab">
      <ReduceOptionsEditor value={o.reduceOptions} onChange={(reduceOptions) => set({ reduceOptions })} />
      <SelectField<BigValueGraphMode>
        label="Graph mode"
        value={o.graphMode}
        options={["none", "line", "area"]}
        onChange={(graphMode) => set({ graphMode })}
      />
      <SelectField<BigValueColorMode>
        label="Color mode"
        value={o.colorMode}
        options={["value", "background", "background_solid", "none"]}
        onChange={(colorMode) => set({ colorMode })}
      />
      <SelectField<BigValueTextMode>
        label="Text mode"
        value={o.textMode}
        options={["auto", "value", "value_and_name", "name", "none"]}
        onChange={(textMode) => set({ textMode })}
      />
      <SelectField<VizOrientation>
        label="Orientation"
        value={o.orientation}
        options={["auto", "horizontal", "vertical"]}
        onChange={(orientation) => set({ orientation })}
      />
    </div>
  );
}
