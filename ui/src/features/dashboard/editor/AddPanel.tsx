// The "Add panel" entry point (viz panel-editor scope) — opens the ONE `PanelEditor` on a FRESH default
// cell (ADD = `cellToEditorState(defaultCell(view))`), the same editor + path EDIT uses. Gated on the
// edit cap by the caller (`canEdit`); the host re-checks `dashboard.save` regardless. On save it appends
// the new cell to the layout. One responsibility: the add affordance + its editor mount.

import { useState } from "react";
import { Plus } from "lucide-react";

import { Button } from "@/components/ui/button";
import type { Cell } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { PanelEditor } from "./PanelEditor";
import { AddLibraryPanel } from "./AddLibraryPanel";
import { defaultCell } from "./defaultCell";

interface Props {
  ws: string;
  existing: Cell[];
  /** Whether the viewer holds `mcp:dashboard.save:call` — when false, no add surface (host is backstop). */
  canEdit: boolean;
  /** Append the new cell (the parent persists the whole dashboard). */
  onAdd: (cell: Cell) => void;
  scope?: VarScope;
}

/** A fresh cell key that doesn't collide with the existing ones (mirrors the v2 builder's `nextKey`). */
function nextKey(existing: Cell[]): string {
  let n = existing.length + 1;
  const keys = new Set(existing.map((c) => c.i));
  while (keys.has(`w${n}`)) n += 1;
  return `w${n}`;
}

export function AddPanel({ ws, existing, canEdit, onAdd, scope }: Props) {
  const [open, setOpen] = useState(false);
  // Build a fresh default timeseries cell when opening, placed below the existing rows.
  const [draft, setDraft] = useState<Cell | null>(null);

  if (!canEdit) return null;

  const openEditor = () => {
    const y = existing.reduce((m, c) => Math.max(m, c.y + c.h), 0);
    setDraft(defaultCell("timeseries", nextKey(existing), { x: 0, y, w: 8, h: 4 }));
    setOpen(true);
  };

  return (
    <div className="flex items-center gap-2 border-b border-border bg-panel px-3 py-2">
      <Button aria-label="add panel" size="sm" onClick={openEditor}>
        <Plus size={12} /> Add panel
      </Button>
      <AddLibraryPanel existing={existing} onAdd={onAdd} />
      {draft && (
        <PanelEditor
          ws={ws}
          cell={draft}
          open={open}
          onOpenChange={setOpen}
          scope={scope}
          onSave={(cell) => onAdd(cell)}
        />
      )}
    </div>
  );
}
