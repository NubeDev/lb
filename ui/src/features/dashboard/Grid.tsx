// The grid host — a `react-grid-layout` of widget cells (dashboard scope). The layout maps 1:1 to the
// dashboard's `cells[]` record; drag/resize stops persist the new geometry via `onLayout` (→
// `dashboard.save`, the SurrealDB record — never localStorage). A measured width (with a sane
// fallback) keeps it deterministic in tests. Read-only viewers get a non-draggable grid.

import { useEffect, useRef, useState } from "react";
import GridLayout, { type Layout } from "react-grid-layout";
import { Copy, Download, GripHorizontal, Pencil, X } from "lucide-react";

import { WidgetHost } from "./WidgetHost";
import { RowHeader } from "./views/RowHeader";
import { canInspect, useDisplayOverride } from "./views/useDisplayOverride";
import type { Cell } from "@/lib/dashboard";
import { isRow, rowMembers, visibleCells } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import type { ExtRow } from "@/lib/ext/ext.api";
import type { DashboardSearch } from "@/features/routing/search";

// react-grid-layout owns positioning; react-resizable owns the visible + hittable resize handle.
import "react-grid-layout/css/styles.css";
import "react-resizable/css/styles.css";

interface Props {
  cells: Cell[];
  editable: boolean;
  range?: DashboardSearch;
  /** Called with the new cell geometry on a drag/resize stop (the persistence seam). */
  onLayout: (cells: Cell[]) => void;
  onRemove: (i: string) => void;
  /** Append a copy of a cell (the persistence seam). */
  onDuplicate: (i: string) => void;
  /** Toggle a row cell's `options.collapsed` (panel-rows). Omitted ⇒ rows are non-collapsible. */
  onToggleRow?: (i: string) => void;
  /** Rename a row cell inline (panel-rows). Omitted ⇒ read-only row title. */
  onRenameRow?: (i: string, title: string) => void;
  /** Edit this panel in the stepped wizard (navigates to `…/new-panel?cell=<i>`, EDIT mode). Called
   *  with the cell key. Omitted ⇒ no button. */
  onEditPanel?: (i: string) => void;
  /** Export this single cell as a widget bundle (`.lbdash.json`). Called with the cell key. Omitted ⇒
   *  no button. Available to viewers too — exporting a definition doesn't widen data access. */
  onExportCell?: (i: string) => void;
  /** Installed extensions (from `ext.list`) — needed to mount `ext:<id>/<widget>` cells. */
  installed?: ExtRow[];
  /** The current workspace (passed to widgets; the hard wall is enforced by the token server-side). */
  workspace?: string;
  /** The resolved variable scope (Slice 3) — interpolated into each cell's calls + ctx. */
  scope?: VarScope;
  /** Auto-refresh tick (Slice 4) — re-runs read cells on each interval. */
  refreshKey?: number;
}

const COLS = 12;
const ROW_H = 56;

export function Grid({
  cells,
  editable,
  range,
  onLayout,
  onRemove,
  onDuplicate,
  onToggleRow,
  onRenameRow,
  onEditPanel,
  onExportCell,
  installed,
  workspace,
  scope,
  refreshKey,
}: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(1200);

  useEffect(() => {
    const measure = () => {
      const w = ref.current?.offsetWidth ?? 0;
      if (w > 0) setWidth(w);
    };

    measure();

    const node = ref.current;
    if (!node || typeof ResizeObserver === "undefined") {
      window.addEventListener("resize", measure);
      return () => window.removeEventListener("resize", measure);
    }

    const observer = new ResizeObserver(measure);
    observer.observe(node);
    return () => observer.disconnect();
  }, []);

  // Only render the VISIBLE cells — a collapsed row's members are dropped from the render list (kept in
  // the record at their real geometry, so expand restores them). The row header itself always renders.
  const shown = visibleCells(cells);

  const layout: Layout[] = shown.map((c) => ({
    i: c.i,
    x: c.x,
    y: c.y,
    w: c.w,
    h: c.h,
    // A row header is a fixed-height full-width bar — it may move but never resize.
    ...(isRow(c) ? { isResizable: false } : {}),
  }));

  // Merge a new layout (geometry only) back onto the cells (which carry binding/options/type). A row
  // that MOVED carries its members: we compute the row's Δy (new `y` − old `y`) and shift every cell
  // that was a positional member of that row BEFORE the move by the same Δy — so a section stays intact
  // when its header is dragged (panel-rows scope, "dragging a row must carry its members"). Members are
  // resolved against the pre-move `cells` (visibleCells shows them for an expanded row; a collapsed
  // row's members are hidden from the layout but still shift, keeping the section contiguous on expand).
  const apply = (next: Layout[]) => {
    const byKey = new Map(next.map((l) => [l.i, l]));
    // Δy per moved row, plus the set of member keys to carry with it.
    const memberShift = new Map<string, number>();
    for (const c of cells) {
      if (!isRow(c)) continue;
      const l = byKey.get(c.i);
      if (!l) continue;
      const dy = l.y - c.y;
      if (dy === 0) continue;
      for (const m of rowMembers(cells, c)) {
        // A member the layout also moved (it was on-screen and react-grid-layout repositioned it) is
        // authoritative from `next`; only carry members the layout did NOT touch (hidden/collapsed).
        if (!byKey.has(m.i)) memberShift.set(m.i, dy);
      }
    }
    onLayout(
      cells.map((c) => {
        const l = byKey.get(c.i);
        if (l) return { ...c, x: l.x, y: l.y, w: l.w, h: l.h };
        const dy = memberShift.get(c.i);
        return dy ? { ...c, y: c.y + dy } : c;
      }),
    );
  };

  return (
    <div
      ref={ref}
      // A faint dot grid marks the canvas as a place things go (the standard authoring-surface
      // affordance) and keeps a sparse board from reading as a dead void. Token-bound + very low
      // alpha so it stays texture, not decoration.
      className="h-full overflow-auto bg-bg bg-[radial-gradient(hsl(var(--fg)/0.055)_1px,transparent_1px)] [background-size:22px_22px] p-4"
      aria-label="dashboard grid"
    >
      <GridLayout
        className="layout"
        layout={layout}
        cols={COLS}
        rowHeight={ROW_H}
        width={width}
        isDraggable={editable}
        isResizable={editable}
        onDragStop={apply}
        onResizeStop={apply}
        draggableHandle=".widget-drag-handle"
        draggableCancel=".widget-no-drag"
      >
        {shown.map((c) =>
          isRow(c) ? (
            // A row header: a full-width, flat, full-bleed section bar — NOT a widget frame (panel-rows
            // scope). The bar owns its own chrome (drag handle + rename + collapse + remove) inline,
            // Grafana-style; the grid item is a bare full-height wrapper so the bar aligns edge-to-edge
            // with the panels below it (no inset gutter).
            <div
              key={c.i}
              data-row=""
              className="group/cell flex h-full flex-col"
              aria-label={`row cell ${c.i}`}
            >
              <RowHeader
                cell={c}
                memberCount={rowMembers(cells, c).length}
                editable={editable}
                onToggleCollapse={(i) => onToggleRow?.(i)}
                onRename={onRenameRow}
                onRemove={onRemove}
              />
            </div>
          ) : (
          <div
            key={c.i}
            data-panel=""
            // A dashboard panel is a raised surface. `data-panel` opts it into the look's Surface
            // treatment (elevated shadow / glass) by cascade. The default (flat) elevation is carried by
            // a crisp border + a 1px inset top-highlight (the Linear/Stripe elevation trick — reads as
            // "lifted" on dark far better than a mushy drop-shadow) rather than a heavy shadow. Hover
            // brightens the border toward the secondary accent. (theme-appearance multi-tone + surfaces.)
            className="surface-panel group/cell flex flex-col overflow-hidden rounded-lg border border-border bg-panel shadow-[inset_0_1px_0_hsl(var(--fg)/0.045),var(--shadow-1)] transition-[box-shadow,border-color] hover:border-fg/25 hover:shadow-[inset_0_1px_0_hsl(var(--fg)/0.06),var(--shadow-2)]"
            aria-label={`cell ${c.i}`}
          >
            <WidgetCell
              cell={c}
              editable={editable}
              range={range}
              installed={installed}
              workspace={workspace}
              scope={scope}
              refreshKey={refreshKey}
              onRemove={onRemove}
              onDuplicate={onDuplicate}
              onEditPanel={onEditPanel}
              onExportCell={onExportCell}
            />
          </div>
          ),
        )}
      </GridLayout>
    </div>
  );
}

/** One non-row widget cell's contents: the hover chrome (move/edit/duplicate/export/remove + the
 *  display-mode toggle) and the widget host. Split out of the map so it can own the per-cell
 *  `useDisplayOverride` hook (rules of hooks). The positioned grid-item div stays in the map so
 *  react-grid-layout keeps cloning a plain element for layout. */
function WidgetCell({
  cell: c,
  editable,
  range,
  installed,
  workspace,
  scope,
  refreshKey,
  onRemove,
  onDuplicate,
  onEditPanel,
  onExportCell,
}: {
  cell: Cell;
  editable: boolean;
  range?: DashboardSearch;
  installed?: ExtRow[];
  workspace?: string;
  scope?: VarScope;
  refreshKey?: number;
  onRemove: (i: string) => void;
  onDuplicate: (i: string) => void;
  onEditPanel?: (i: string) => void;
  onExportCell?: (i: string) => void;
}) {
  const display = useDisplayOverride();
  const DisplayIcon = display.icon;
  return (
    <>
      {/* Display-mode toggle (viz → table → JSON → viz) — view-only, never persisted. Available to
          VIEWERS too (it widens no data access; it re-renders the same frames the cell already read),
          so it lives OUTSIDE the `editable` chrome. Only shown for read views that resolve frames. */}
      {canInspect(c) && (
        <div className="widget-no-drag absolute right-2 top-2 z-20 flex items-center">
          <button
            type="button"
            aria-label={`toggle display mode for cell ${c.i}`}
            aria-pressed={display.override !== null}
            title={display.title}
            className="inline-flex h-6 w-6 items-center justify-center rounded-md text-muted opacity-0 transition-[opacity,color,background-color] hover:bg-panel-2 hover:text-fg focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/30 group-hover/cell:opacity-100 data-[on=true]:opacity-100 data-[on=true]:text-accent"
            data-on={display.override !== null}
            onClick={() => display.cycle()}
          >
            <DisplayIcon size={13} />
          </button>
        </div>
      )}
      {/* Edit affordances reveal on hover/focus (cleaner default; the tell of a polished board is a
          quiet resting state). Keyboard focus still surfaces them. */}
      {editable && (
        <button
          type="button"
          aria-label={`move cell ${c.i}`}
          title="Move widget"
          className="widget-drag-handle absolute left-2 top-2 z-10 inline-flex h-6 w-6 cursor-grab items-center justify-center rounded-md text-muted opacity-0 transition-[opacity,color,background-color] hover:bg-panel-2 hover:text-fg focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/30 active:cursor-grabbing group-hover/cell:opacity-100"
        >
          <GripHorizontal size={13} />
        </button>
      )}
      {editable && (
        <div className={`widget-no-drag absolute ${canInspect(c) ? "right-9" : "right-2"} top-2 z-10 flex items-center gap-0.5 opacity-0 transition-[opacity] focus-within:opacity-100 group-hover/cell:opacity-100`}>
          {onEditPanel && (
                  <button
                    aria-label={`edit cell ${c.i}`}
                    title="Edit panel"
                    className="inline-flex h-6 w-6 items-center justify-center rounded-md text-muted transition-[color,background-color] hover:bg-panel-2 hover:text-fg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/30"
                    onClick={() => onEditPanel(c.i)}
                  >
                    <Pencil size={13} />
                  </button>
                )}
                <button
                  aria-label={`duplicate cell ${c.i}`}
                  title="Duplicate widget"
                  className="inline-flex h-6 w-6 items-center justify-center rounded-md text-muted transition-[color,background-color] hover:bg-panel-2 hover:text-fg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/30"
                  onClick={() => onDuplicate(c.i)}
                >
                  <Copy size={13} />
                </button>
                {onExportCell && (
                  <button
                    aria-label={`export cell ${c.i}`}
                    title="Export widget"
                    className="inline-flex h-6 w-6 items-center justify-center rounded-md text-muted transition-[color,background-color] hover:bg-panel-2 hover:text-fg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/30"
                    onClick={() => onExportCell(c.i)}
                  >
                    <Download size={13} />
                  </button>
                )}
                <button
                  aria-label={`remove cell ${c.i}`}
                  title="Remove widget"
                  className="inline-flex h-6 w-6 items-center justify-center rounded-md text-muted transition-[color,background-color] hover:bg-destructive/12 hover:text-destructive focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-destructive/30"
                  onClick={() => onRemove(c.i)}
                >
                  <X size={13} />
                </button>
              </div>
            )}
      <div className="min-h-0 flex-1 p-3">
        <WidgetHost
          cell={display.applyTo(c)}
          range={range}
          installed={installed}
          workspace={workspace}
          scope={scope}
          refreshKey={refreshKey}
        />
      </div>
    </>
  );
}
