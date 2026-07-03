// The `gauge` per-viz options editor — registry-driven (editor-parity step 5): threshold labels/markers
// + orientation from the registry; the reduceOptions calc keeps its bespoke chip editor. One
// responsibility: compose the gauge per-viz options.

import type { EditorState } from "../../cellEditorState";
import { readGaugeOptions } from "../../../views/gauge/options";
import { VizOptions } from "./VizOptions";
import { ReduceOptionsEditor } from "./ReduceOptionsEditor";

export function GaugeOptionsEditor({ state, patch }: { state: EditorState; patch: (n: Partial<EditorState>) => void }) {
  const o = readGaugeOptions(state.options);
  return (
    <VizOptions
      view="gauge"
      state={state}
      patch={patch}
      extras={<ReduceOptionsEditor value={o.reduceOptions} onChange={(reduceOptions) => patch({ options: { ...state.options, reduceOptions } })} />}
    />
  );
}
