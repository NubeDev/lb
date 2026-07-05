// The Sources rail tab (data-studio scope) — the shipped `@nube/source-picker` over every source
// type (series / Direct SurrealDB / flows / installed extensions / federation datasources). Picking a
// source OPENS A STACKED BUILDER TAB in the workbench. One responsibility: the picker + the
// "pick → open tab" seam. Rendered inside `StudioRail` (the rail body owns padding/scroll).

import { SourcePicker, READ_SOURCE_GROUPS, type SourceEntry, type SourceSelection } from "@nube/source-picker";

interface Props {
  entries: SourceEntry[];
  loading: boolean;
  /** Open a builder tab on the selection (`label` is the picked entry's friendly label — the tab name). */
  onOpen: (sel: SourceSelection, label: string) => void;
}

export function SourcesPane({ entries, loading, onOpen }: Props) {
  return (
    <div className="flex flex-col gap-2">
      <p className="px-1 text-xs text-muted">Pick a source to open it in a builder tab.</p>
      <SourcePicker
        aria-label="explore source"
        entries={entries}
        loading={loading}
        groups={READ_SOURCE_GROUPS}
        onSelect={(sel) => {
          if (!sel) return;
          const label = entries.find((e) => e.id === sel.id)?.label ?? "Explore";
          onOpen(sel, label);
        }}
      />
    </div>
  );
}
