// A panel-rows section header (panel-rows scope) — a flat, full-bleed Grafana-style section bar:
// [drag grip on hover] chevron  Title  · N panels  … … …  [remove on hover]. This is a LAYOUT view:
// the grid special-cases `view:"row"` to render this instead of a widget frame, and folds/unfolds the
// member cells beneath it. Clicking anywhere on the bar (outside a control) toggles collapse — the
// whole header is the affordance, as Grafana does. Collapse writes `options.collapsed` on the row cell
// (persisted via `dashboard.save`); the count is the number of positional members (`rowMembers`).
// Editable only when the board is editable.

import { useState } from "react";
import { ChevronDown, ChevronRight, GripVertical, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { Cell } from "@/lib/dashboard";
import { cellLabel, isCollapsed } from "@/lib/dashboard";

interface Props {
  cell: Cell;
  /** How many positional members this row owns (shown beside the title). */
  memberCount: number;
  editable: boolean;
  /** Toggle `options.collapsed` (the persistence seam). */
  onToggleCollapse: (i: string) => void;
  /** Rename the row inline (the persistence seam). Omitted / non-editable ⇒ read-only title. */
  onRename?: (i: string, title: string) => void;
  /** Remove the row header (row-only delete). Omitted / non-editable ⇒ no remove affordance. */
  onRemove?: (i: string) => void;
}

export function RowHeader({
  cell,
  memberCount,
  editable,
  onToggleCollapse,
  onRename,
  onRemove,
}: Props) {
  const collapsed = isCollapsed(cell);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const label = cellLabel(cell);

  const commit = () => {
    const next = draft.trim();
    if (next && next !== label) onRename?.(cell.i, next);
    setEditing(false);
  };

  return (
    // A flat section divider (Grafana) — a subtle bottom border, no card. `h-full` fills the grid row
    // exactly so the bar's edges align with the panels beneath. The chevron + title is ONE real button
    // that toggles collapse; the grip + remove are siblings so no interactive element nests in another.
    <div
      data-row-header=""
      className="group/row flex h-full w-full select-none items-center gap-1 border-b border-border/80 pl-1 pr-2"
      aria-label={`row ${label}`}
    >
      {editable && (
        // Drag grip — reveals on hover, far left. It is the grid's draggable handle for the row.
        <span
          aria-label={`move cell ${cell.i}`}
          title="Move row"
          className="widget-drag-handle flex h-6 w-4 shrink-0 cursor-grab items-center justify-center text-muted opacity-0 transition-opacity group-hover/row:opacity-100 active:cursor-grabbing"
        >
          <GripVertical size={13} />
        </span>
      )}
      {editing && editable ? (
        <>
          <span className="flex h-5 w-5 shrink-0 items-center justify-center text-muted">
            <ChevronDown size={16} />
          </span>
          <Input
            autoFocus
            aria-label="row title"
            className="widget-no-drag h-6 min-w-0 max-w-xs flex-1 text-sm font-semibold"
            defaultValue={label}
            onChange={(e) => setDraft(e.target.value)}
            onBlur={commit}
            onKeyDown={(e) => {
              if (e.key === "Enter") commit();
              if (e.key === "Escape") setEditing(false);
            }}
          />
        </>
      ) : (
        // The collapse toggle spans the chevron + title + count and stretches across the free space so
        // clicking anywhere on the bar collapses it (Grafana). Double-click enters rename.
        <Button
          type="button"
          variant="ghost"
          aria-label={collapsed ? `expand row ${label}` : `collapse row ${label}`}
          aria-expanded={!collapsed}
          title={collapsed ? "Expand row" : "Collapse row"}
          className="flex h-full min-w-0 flex-1 items-center justify-start gap-1.5 rounded-md px-1 text-left font-normal hover:bg-panel-2/40"
          onClick={() => onToggleCollapse(cell.i)}
          onDoubleClick={(e) => {
            if (!editable || !onRename) return;
            e.stopPropagation();
            setDraft(label);
            setEditing(true);
          }}
        >
          <span className="flex h-5 w-5 shrink-0 items-center justify-center text-muted">
            {collapsed ? <ChevronRight size={16} /> : <ChevronDown size={16} />}
          </span>
          <span className="truncate text-sm font-semibold text-fg">{label}</span>
          {memberCount > 0 && (
            <span className="shrink-0 text-xs font-normal text-muted">
              · {memberCount} panel{memberCount === 1 ? "" : "s"}
            </span>
          )}
        </Button>
      )}
      {editable && onRemove && (
        <Button
          type="button"
          variant="ghost"
          size="icon"
          aria-label={`remove cell ${cell.i}`}
          title="Remove row"
          className="widget-no-drag h-6 w-6 shrink-0 text-muted opacity-0 transition-opacity hover:bg-destructive/12 hover:text-destructive focus-visible:opacity-100 group-hover/row:opacity-100"
          onClick={() => onRemove(cell.i)}
        >
          <X size={13} />
        </Button>
      )}
    </div>
  );
}
