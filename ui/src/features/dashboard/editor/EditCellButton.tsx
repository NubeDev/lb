// The per-cell edit affordance (viz panel-editor scope) — the ⚙ button that opens the ONE `PanelEditor`
// on the EXISTING cell (EDIT = `cellToEditorState(savedCell)`, the same editor + path ADD uses). It
// REPLACES the retired `CellSettings` ⚙ drawer (a single editor is the whole point — Resolved decision:
// "Retire CellSettings"). On save it writes the rebuilt cell back (key + geometry preserved by the
// serializer). Gated on the edit cap by the caller. One responsibility: the per-cell edit entry point.

import { useState } from "react";
import { Settings } from "lucide-react";

import { Button } from "@/components/ui/button";
import type { Cell } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { PanelEditor } from "./PanelEditor";

interface Props {
  ws: string;
  cell: Cell;
  /** Write the edited cell back (the parent splices it into the layout + saves). */
  onSave: (cell: Cell) => void;
  scope?: VarScope;
}

export function EditCellButton({ ws, cell, onSave, scope }: Props) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button
        variant="ghost"
        aria-label={`edit cell ${cell.i}`}
        title="Edit panel"
        className="widget-no-drag absolute right-7 top-1.5 z-10 h-auto rounded-md p-1 text-muted hover:bg-accent/10 hover:text-accent"
        onClick={() => setOpen(true)}
      >
        <Settings size={12} />
      </Button>
      <PanelEditor ws={ws} cell={cell} open={open} onOpenChange={setOpen} scope={scope} onSave={onSave} />
    </>
  );
}
