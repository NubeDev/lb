// The `table` per-viz options editor (viz panel-editor scope) — now registry-driven (editor-parity step
// 5): showHeader / cellHeight / pagination / footer come from the option registry via VizOptions. The
// per-COLUMN options (width/alignment/cell type/filter) are `fieldConfig.custom.*` and live in the
// table's Field tab (Grafana's line). One responsibility: compose the table per-viz options.

import type { EditorState } from "../../cellEditorState";
import { VizOptions } from "./VizOptions";

export function TableOptionsEditor({ state, patch }: { state: EditorState; patch: (n: Partial<EditorState>) => void }) {
  return <VizOptions view="table" state={state} patch={patch} />;
}
