// The `bargauge` per-viz options editor — registry-driven (editor-parity step 5): display mode +
// show-unfilled + orientation from the registry; reduceOptions calc as a bespoke extra. One
// responsibility: compose the bargauge per-viz options.

import type { EditorState } from "@/lib/panel-kit/cellEditorState";
import { readBarGaugeOptions } from "@/features/dashboard/views/bargauge/options";
import { VizOptions } from "./VizOptions";
import { ReduceOptionsEditor } from "./ReduceOptionsEditor";

export function BarGaugeOptionsEditor({ state, patch }: { state: EditorState; patch: (n: Partial<EditorState>) => void }) {
  const o = readBarGaugeOptions(state.options);
  return (
    <VizOptions
      view="bargauge"
      state={state}
      patch={patch}
      extras={<ReduceOptionsEditor value={o.reduceOptions} onChange={(reduceOptions) => patch({ options: { ...state.options, reduceOptions } })} />}
    />
  );
}
