// The `stat` per-viz options editor (viz panel-editor scope) — now registry-driven (editor-parity step
// 5): the value-display options (color/graph/text/justify mode, orientation, percent-change) come from
// the option registry via VizOptions; the reduceOptions calc keeps its bespoke chip editor as an extra.
// One responsibility: compose the stat per-viz options.

import type { EditorState } from "@/lib/panel-kit/cellEditorState";
import { readStatOptions } from "@/features/dashboard/views/stat/options";
import { VizOptions } from "./VizOptions";
import { ReduceOptionsEditor } from "./ReduceOptionsEditor";

export function StatOptionsEditor({ state, patch }: { state: EditorState; patch: (n: Partial<EditorState>) => void }) {
  const o = readStatOptions(state.options);
  return (
    <VizOptions
      view="stat"
      state={state}
      patch={patch}
      extras={
        <ReduceOptionsEditor value={o.reduceOptions} onChange={(reduceOptions) => patch({ options: { ...state.options, reduceOptions } })} />
      }
    />
  );
}
