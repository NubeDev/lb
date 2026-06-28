// The per-cell settings drawer (widget-config-vars scope, Slice 1: "edit, not re-add"). A ⚙ button on
// each cell in edit mode opens this Sheet, which reuses the WidgetBuilder's source/view/option fields in
// an "edit existing cell" mode (seeded from the cell, written back on save). Saving rebuilds the cell
// keeping its key + geometry and persists the WHOLE dashboard via the existing `saveCells`/`dashboard.save`
// — no new verb. The affordance is gated on the edit cap (the widget-palette gate, reused); the host
// re-checks `dashboard.save` regardless.

import { useState } from "react";
import { Settings } from "lucide-react";

import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import type { Cell } from "@/lib/dashboard";
import { cellLabel } from "@/lib/dashboard";
import { WidgetBuilder } from "./WidgetBuilder";
import { JsonPayloadField } from "./JsonPayloadField";
import { useSourcePicker } from "./useSourcePicker";

interface Props {
  ws: string;
  /** The cell being edited (seeds the builder fields). */
  cell: Cell;
  /** All cells, so the builder's "existing" geometry math is consistent (it keeps this cell's key). */
  existing: Cell[];
  /** Open state controlled by the cell's ⚙ button. */
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Write the edited cell back (the parent splices it into the layout + persists). */
  onSave: (cell: Cell) => void;
}

/** The settings drawer for one cell. Renders the builder in edit mode (`seed`/`onSave`) inside a Sheet,
 *  plus — for a control cell — the JSON payload builder (Slice 5) so its send body can be authored + tested. */
export function CellSettings({ ws, cell, existing, open, onOpenChange, onSave }: Props) {
  const { installed } = useSourcePicker(ws);
  // A control cell (button/switch/slider) can author a JSON payload to a write/bus target.
  const isControl = ["button", "switch", "slider"].includes(cell.view ?? "");
  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent side="right" className="w-full overflow-y-auto sm:max-w-md" aria-label="cell settings">
        <SheetHeader>
          <SheetTitle>Widget settings</SheetTitle>
          <SheetDescription>Edit “{cellLabel(cell)}” — source, view, options, and title.</SheetDescription>
        </SheetHeader>
        <div className="flex flex-col gap-4 px-4 pb-4">
          <WidgetBuilder
            ws={ws}
            existing={existing}
            canEdit
            bare
            seed={cell}
            onAdd={() => {}}
            onSave={(c) => {
              onSave(c);
              onOpenChange(false);
            }}
          />
          {isControl && (
            <div className="border-t border-border pt-3">
              <div className="mb-2 font-medium text-muted">JSON payload</div>
              <JsonPayloadField ws={ws} installed={installed} />
            </div>
          )}
        </div>
      </SheetContent>
    </Sheet>
  );
}

/** A small per-cell ⚙ button that opens its settings drawer. Owns the open state so the Grid stays thin. */
export function CellSettingsButton({
  ws,
  cell,
  existing,
  onSave,
}: {
  ws: string;
  cell: Cell;
  existing: Cell[];
  onSave: (cell: Cell) => void;
}) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button
        aria-label={`settings cell ${cell.i}`}
        title="Widget settings"
        className="widget-no-drag absolute right-7 top-1.5 z-10 rounded-md p-1 text-muted transition-colors hover:bg-accent/10 hover:text-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/25"
        onClick={() => setOpen(true)}
      >
        <Settings size={12} />
      </button>
      <CellSettings
        ws={ws}
        cell={cell}
        existing={existing}
        open={open}
        onOpenChange={setOpen}
        onSave={onSave}
      />
    </>
  );
}
