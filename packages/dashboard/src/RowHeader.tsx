// A panel-rows section header — a flat, full-bleed Grafana-style section bar:
// [drag grip on hover] chevron  Title  · N panels  …  [remove on hover]. The grid special-cases
// `view:"row"` to render this instead of a widget frame and folds/unfolds the member cells
// beneath it. Clicking the bar toggles collapse; double-click enters inline rename (when the
// board is editable and `onRename` is passed). Ported from the shell's RowHeader with the
// shadcn Button/Input and the options popout dialog cut — plain elements on `lbdg-*` classes;
// the `collapsed`/rename persistence callbacks are the package seam.

import { useState } from "react";
import { ChevronDown, ChevronRight, GripVertical, X } from "lucide-react";

import type { Cell } from "./dashboard.types";
import { cellLabel } from "./dashboard.types";
import { isCollapsed, rowOptions } from "./rows";

export interface RowHeaderProps {
  cell: Cell;
  /** How many positional members this row owns (shown beside the title). */
  memberCount: number;
  editable: boolean;
  /** Toggle `options.collapsed` (the persistence seam). Omitted ⇒ the chevron is inert. */
  onToggleCollapse?: (i: string) => void;
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
}: RowHeaderProps) {
  const collapsed = isCollapsed(cell);
  const opts = rowOptions(cell);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const label = cellLabel(cell);

  const commit = () => {
    const next = draft.trim();
    if (next && next !== label) onRename?.(cell.i, next);
    setEditing(false);
  };

  return (
    <div
      data-row-header=""
      className={`lbdg-row-header${opts.showLine ? " lbdg-row-header--line" : ""}`}
      aria-label={`row ${label}`}
    >
      {editable && (
        <span
          aria-label={`move cell ${cell.i}`}
          title="Move row"
          className="lbdg-drag-handle lbdg-row-grip"
        >
          <GripVertical size={13} />
        </span>
      )}
      {editing && editable ? (
        <>
          <span className="lbdg-row-chevron">
            <ChevronDown size={16} />
          </span>
          <input
            autoFocus
            aria-label="row title"
            className="lbdg-no-drag lbdg-row-rename"
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
        <button
          type="button"
          aria-label={collapsed ? `expand row ${label}` : `collapse row ${label}`}
          aria-expanded={!collapsed}
          title={collapsed ? "Expand row" : "Collapse row"}
          className="lbdg-row-toggle"
          onClick={() => onToggleCollapse?.(cell.i)}
          onDoubleClick={(e) => {
            if (!editable || !onRename) return;
            e.stopPropagation();
            setDraft(label);
            setEditing(true);
          }}
        >
          <span className="lbdg-row-chevron">
            {collapsed ? <ChevronRight size={16} /> : <ChevronDown size={16} />}
          </span>
          <span className="lbdg-row-title">{label}</span>
          {opts.showCount && memberCount > 0 && (
            <span className="lbdg-row-count">
              · {memberCount} panel{memberCount === 1 ? "" : "s"}
            </span>
          )}
        </button>
      )}
      {editable && onRemove && (
        <button
          type="button"
          aria-label={`remove cell ${cell.i}`}
          title="Remove row"
          className="lbdg-no-drag lbdg-btn lbdg-btn--danger lbdg-row-remove"
          onClick={() => onRemove(cell.i)}
        >
          <X size={13} />
        </button>
      )}
    </div>
  );
}
