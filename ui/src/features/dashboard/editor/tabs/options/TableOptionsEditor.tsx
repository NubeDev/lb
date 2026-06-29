// The `table` per-viz options editor (viz panel-editor scope). Edits the typed `TableOptions` —
// showHeader + cellHeight + pagination. (Per-column cell display + sortBy UI are field-config/Phase-3
// follow-ups; sortBy round-trips as data.) One responsibility: edit a table cell's `options`.

import type { EditorState } from "../../cellEditorState";
import { readTableOptions, type TableCellHeight } from "../../../views/table/options";
import { SelectField, ToggleField } from "./controls";

export function TableOptionsEditor({ state, patch }: { state: EditorState; patch: (n: Partial<EditorState>) => void }) {
  const o = readTableOptions(state.options);
  const set = (next: Partial<typeof o>) => patch({ options: { ...state.options, ...next } });

  return (
    <div className="grid gap-3 py-3 text-xs" aria-label="panel options tab">
      <ToggleField label="Show header" checked={o.showHeader} onChange={(showHeader) => set({ showHeader })} />
      <SelectField<TableCellHeight>
        label="Cell height"
        value={o.cellHeight}
        options={["sm", "md", "lg"]}
        onChange={(cellHeight) => set({ cellHeight })}
      />
      <ToggleField label="Enable pagination" checked={o.enablePagination} onChange={(enablePagination) => set({ enablePagination })} />
    </div>
  );
}
