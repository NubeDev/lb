// The row-options popout (panel-rows options). A modal dialog — NOT an inline popover — over a row
// header's three presentation toggles: show the panel count, show the divider line, and the default
// open/closed (collapsed) state. Editing is local; Save writes the whole `options` block back through
// the caller's `onSave` (→ `dashboard.save`). The row view carries no fieldConfig and no data, so this
// is the row's entire config surface (the panel wizard's Options step renders the same three defs from
// the registry — `defs/row.ts` — this dialog is the in-context entry). One responsibility: the row
// options interaction.

import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Switch } from "@/components/ui/switch";
import type { Cell, RowOptions } from "@/lib/dashboard";
import { cellLabel, rowOptions } from "@/lib/dashboard";

interface Props {
  cell: Cell;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Persist the row's new presentation options (the `options` merge → `dashboard.save`). */
  onSave: (i: string, options: RowOptions) => void;
}

/** One labelled toggle row. */
function ToggleRow({
  id,
  label,
  hint,
  checked,
  onChange,
}: {
  id: string;
  label: string;
  hint: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <label
      htmlFor={id}
      className="flex cursor-pointer items-start justify-between gap-4 rounded-md border border-border/60 bg-panel-2/40 px-3 py-2.5"
    >
      <span className="min-w-0">
        <span className="block text-sm font-medium text-fg">{label}</span>
        <span className="block text-xs text-muted">{hint}</span>
      </span>
      <Switch id={id} checked={checked} onCheckedChange={onChange} aria-label={label} />
    </label>
  );
}

export function RowOptionsDialog({ cell, open, onOpenChange, onSave }: Props) {
  const [opts, setOpts] = useState<RowOptions>(() => rowOptions(cell));

  // Re-seed from the cell each time the dialog opens (a fresh edit session starts from the stored value,
  // never a stale local draft).
  useEffect(() => {
    if (open) setOpts(rowOptions(cell));
  }, [open, cell]);

  const save = () => {
    onSave(cell.i, opts);
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>Row options — “{cellLabel(cell)}”</DialogTitle>
          <DialogDescription>
            How this section header looks and behaves. Applies on save.
          </DialogDescription>
        </DialogHeader>

        <div className="flex flex-col gap-2">
          <ToggleRow
            id="row-opt-count"
            label="Show panel count"
            hint="Display the “· N panels” member count beside the title."
            checked={opts.showCount}
            onChange={(showCount) => setOpts((o) => ({ ...o, showCount }))}
          />
          <ToggleRow
            id="row-opt-line"
            label="Show divider line"
            hint="Draw the horizontal rule under the row header."
            checked={opts.showLine}
            onChange={(showLine) => setOpts((o) => ({ ...o, showLine }))}
          />
          <ToggleRow
            id="row-opt-collapsed"
            label="Collapsed by default"
            hint="The row loads folded — its panels are hidden until expanded."
            checked={opts.collapsed}
            onChange={(collapsed) => setOpts((o) => ({ ...o, collapsed }))}
          />
        </div>

        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={save}>Save</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
