// The Sources dock pane (data-studio scope v2) — the shipped `@nube/source-picker` over every source
// type (series / Direct SurrealDB / flows / installed extensions / federation datasources). Picking a
// source OPENS A NEW EXPLORE TAB in the workbench (the multi-pane correction over v1's single
// preview). One responsibility: the picker + the "pick → open tab" seam.

import { SourcePicker, READ_SOURCE_GROUPS, type SourceEntry, type SourceSelection } from "@nube/source-picker";

interface Props {
  entries: SourceEntry[];
  loading: boolean;
  /** Open an explore tab on the selection (`label` is the picked entry's friendly label — the tab name). */
  onOpen: (sel: SourceSelection, label: string) => void;
}

export function SourcesPane({ entries, loading, onOpen }: Props) {
  return (
    <div className="flex h-full min-h-0 flex-col gap-2 overflow-y-auto p-2">
      <p className="text-xs text-muted">Pick a source to open it in an explore tab.</p>
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
