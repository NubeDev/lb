// The grid host — a `react-grid-layout` of widget cells over a persisted `cells[]` record.
// Ported from the shell's `features/dashboard/Grid.tsx` with the four shell entanglements cut
// behind the package boundary: WidgetHost dispatch → the consumer's widget REGISTRY, VarScope →
// the opaque generic `scope`, DashboardSearch → the package `TimeRange`, ExtRow/`ext:` mounting
// → an ordinary registry entry. Drag/resize stops persist the new geometry via `onLayout` (the
// persistence seam — the package itself persists NOTHING). A measured width (with the 1200px
// fallback) keeps it deterministic in tests. Read-only viewers get a non-draggable grid; below
// `stackBelow` px the board degrades to the read-only single-column stack (mobile).

import { useEffect, useRef, useState } from "react";
import GridLayout, { type Layout } from "react-grid-layout";
import { Copy, Download, GripHorizontal, Link as LinkIcon, Pencil, X } from "lucide-react";

import type { Cell } from "./dashboard.types";
import { cellView, GRID_COLS, GRID_ROW_PX } from "./dashboard.types";
import { isRow, rowMembers, visibleCells } from "./rows";
import { mergeLayout } from "./layout";
import type { WidgetRegistry } from "./registry";
import { UnknownView } from "./registry";
import { RowHeader } from "./RowHeader";
import { DashboardStack } from "./Stack";
import type { TimeRange } from "./timerange";
import { timeOverrideBadge } from "./timeOverrideBadge";

/** The deterministic width used before the container has been measured (and in jsdom tests). */
export const FALLBACK_WIDTH = 1200;

/** react-grid-layout's resize-handle axes. Mirrors RGL's own `ResizeHandle` union, which its
 *  types declare but do not export from the module root — so we spell it here rather than import. */
export type ResizeHandle = "s" | "w" | "e" | "n" | "sw" | "nw" | "se" | "ne";

/** The resize grips an editable widget offers by default — all four corners plus all four edges,
 *  so a widget resizes from whichever side the user grabs (react-grid-layout's own default is the
 *  SE corner alone). Rows never resize (see the per-cell override in the layout map). */
export const DEFAULT_RESIZE_HANDLES: ResizeHandle[] = ["s", "w", "e", "n", "sw", "nw", "se", "ne"];

export interface DashboardGridProps<S = unknown> {
  cells: Cell[];
  editable: boolean;
  /** The consumer's view → renderer map. An unregistered view renders the honest placeholder. */
  registry: WidgetRegistry<S>;
  /** The dashboard's active time window, passed through to every renderer. */
  range?: TimeRange;
  /** The opaque consumer scope (variables etc.), passed through to every renderer. */
  scope?: S;
  /** Auto-refresh tick, passed through to every renderer. */
  refreshKey?: number;
  /** Called with the new cell geometry on a drag/resize stop (the persistence seam). */
  onLayout: (cells: Cell[]) => void;
  /** Remove a cell. Omitted ⇒ no remove affordance. */
  onRemove?: (i: string) => void;
  /** Append a copy of a cell. Omitted ⇒ no duplicate affordance. */
  onDuplicate?: (i: string) => void;
  /** Toggle a row cell's `options.collapsed` (panel-rows). Omitted ⇒ rows are non-collapsible. */
  onToggleRow?: (i: string) => void;
  /** Rename a row cell inline (panel-rows). Omitted ⇒ read-only row title. */
  onRenameRow?: (i: string, title: string) => void;
  /** Edit this panel (the consumer navigates to its editor). Omitted ⇒ no button. */
  onEditPanel?: (i: string) => void;
  /** Export this single cell. Available to viewers too — exporting a definition doesn't widen
   *  data access. Omitted ⇒ no button. */
  onExportCell?: (i: string) => void;
  /** Below this measured width (px) the board renders as the read-only mobile stack. Default
   *  768 ("below md"); pass 0 to always render the grid. */
  stackBelow?: number;
  /** Which resize grips an editable widget offers. Default {@link DEFAULT_RESIZE_HANDLES}
   *  (every corner + edge); pass `["se"]` for the SE-corner-only classic behaviour. Ignored
   *  when the board is read-only (`editable` false) and on row-header cells (never resizable). */
  resizeHandles?: ResizeHandle[];
  /** Accept EXTERNAL drags (react-grid-layout's drop seam): the consumer marks its palette item
   *  `draggable` and sets a `dataTransfer` payload; the grid previews `droppingItem` while the
   *  drag hovers and calls `onDrop` with the landed slot. Only honored while `editable`. */
  droppable?: boolean;
  /** The placeholder geometry previewed while an external drag hovers the grid. */
  droppingItem?: { i: string; w: number; h: number };
  /** An external draggable landed: the grid slot it occupies + the native drag event (the
   *  consumer reads its own payload off `event.dataTransfer`). */
  onDrop?: (slot: { x: number; y: number; w: number; h: number }, event: DragEvent) => void;
}

export function DashboardGrid<S = unknown>({
  cells,
  editable,
  registry,
  range,
  scope,
  refreshKey,
  onLayout,
  onRemove,
  onDuplicate,
  onToggleRow,
  onRenameRow,
  onEditPanel,
  onExportCell,
  stackBelow = 768,
  resizeHandles = DEFAULT_RESIZE_HANDLES,
  droppable,
  droppingItem,
  onDrop,
}: DashboardGridProps<S>) {
  const ref = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(FALLBACK_WIDTH);

  useEffect(() => {
    const measure = () => {
      const node = ref.current;
      if (!node) return;
      // react-grid-layout draws in the CONTENT box — the canvas padding must be
      // excluded or the grid overflows by exactly that padding (a permanent
      // horizontal scrollbar). clientWidth also excludes any vertical scrollbar.
      const style = window.getComputedStyle(node);
      const w =
        node.clientWidth -
        (parseFloat(style.paddingLeft) || 0) -
        (parseFloat(style.paddingRight) || 0);
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

  // Only render the VISIBLE cells — a collapsed row's members are dropped from the render list
  // (kept in the record at their real geometry, so expand restores them). Rows always render.
  const shown = visibleCells(cells);

  const layout: Layout[] = shown.map((c) => ({
    i: c.i,
    x: c.x,
    y: c.y,
    w: c.w,
    h: c.h,
    // A row header is a fixed-height full-width bar — it may move but never resize. Widget cells
    // clamp resizing to their per-cell minimums (absent ⇒ react-grid-layout's 1×1 default), so a
    // chart can't be dragged down to an unreadable sliver, and carry the grip set PER ITEM: RGL
    // renders a handle span for every axis in a grid-level `resizeHandles` even where the item is
    // non-resizable, so a read-only board or a row would sprout dead grips — set them here where
    // `editable`/`isRow` already gate the rest of the item's behaviour.
    ...(isRow(c)
      ? { isResizable: false, resizeHandles: [] as ResizeHandle[] }
      : {
          resizeHandles: editable ? resizeHandles : ([] as ResizeHandle[]),
          ...(c.minW !== undefined ? { minW: c.minW } : {}),
          ...(c.minH !== undefined ? { minH: c.minH } : {}),
        }),
  }));

  const apply = (next: Layout[]) => onLayout(mergeLayout(cells, next));

  // Below the breakpoint: the same cells as a read-only single-column stack (mobile).
  if (stackBelow > 0 && width < stackBelow) {
    return (
      <div ref={ref} className="lbdg-root" aria-label="dashboard grid">
        <DashboardStack cells={cells} registry={registry} range={range} scope={scope} refreshKey={refreshKey} />
      </div>
    );
  }

  const isDroppable = Boolean(droppable && editable && onDrop);

  return (
    <div
      ref={ref}
      className="lbdg-root lbdg-canvas"
      aria-label="dashboard grid"
      // Lets the stylesheet give an EMPTY droppable grid a landing area (RGL's inline
      // height is 0 with no rows, which would make the first drop impossible).
      data-droppable={isDroppable ? "true" : undefined}
    >
      <GridLayout
        className="layout"
        layout={layout}
        cols={GRID_COLS}
        rowHeight={GRID_ROW_PX}
        width={width}
        isDraggable={editable}
        isResizable={editable}
        onDragStop={apply}
        onResizeStop={apply}
        draggableHandle=".lbdg-drag-handle"
        draggableCancel=".lbdg-no-drag"
        isDroppable={isDroppable}
        droppingItem={droppingItem}
        onDrop={(_next, item, e) => {
          if (item && onDrop) onDrop({ x: item.x, y: item.y, w: item.w, h: item.h }, e as unknown as DragEvent);
        }}
      >
        {shown.map((c) =>
          isRow(c) ? (
            // A row header: a full-width, flat, full-bleed section bar — NOT a widget frame. The
            // bar owns its own chrome (drag handle + rename + collapse + remove) inline.
            <div key={c.i} data-row="" className="lbdg-row-item" aria-label={`row cell ${c.i}`}>
              <RowHeader
                cell={c}
                memberCount={rowMembers(cells, c).length}
                editable={editable}
                onToggleCollapse={onToggleRow}
                onRename={onRenameRow}
                onRemove={onRemove}
              />
            </div>
          ) : (
            <div
              key={c.i}
              // Grafana's `transparent` panel: NO chrome at all — the panel sits directly on the
              // board. A half-dropped chrome (bg gone but border kept) is the broken look; drop it all.
              className={c.transparent ? "lbdg-cell lbdg-cell--transparent" : "lbdg-cell lbdg-cell--framed"}
              data-transparent={c.transparent ? "true" : undefined}
              aria-label={`cell ${c.i}`}
            >
              <WidgetCell
                cell={c}
                editable={editable}
                registry={registry}
                range={range}
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

/** One non-row widget cell's contents: the hover chrome (move/edit/duplicate/export/remove),
 *  the time-override badge + panel links, and the registry-dispatched renderer. Split out of
 *  the map so react-grid-layout keeps cloning a plain positioned div for layout. */
function WidgetCell<S>({
  cell: c,
  editable,
  registry,
  range,
  scope,
  refreshKey,
  onRemove,
  onDuplicate,
  onEditPanel,
  onExportCell,
}: {
  cell: Cell;
  editable: boolean;
  registry: WidgetRegistry<S>;
  range?: TimeRange;
  scope?: S;
  refreshKey?: number;
  onRemove?: (i: string) => void;
  onDuplicate?: (i: string) => void;
  onEditPanel?: (i: string) => void;
  onExportCell?: (i: string) => void;
}) {
  // The badge + links are VIEWER-facing (they widen no data access), so they live outside the
  // `editable` chrome, exactly as in the shell.
  const badge = timeOverrideBadge(c.queryOptions);
  const links = c.links ?? [];
  const view = cellView(c);
  const Renderer = registry.resolveCell(c);
  return (
    <>
      {/* A panel whose range differs from the dashboard's SAYS so, or a viewer silently
          misreads a shifted panel as "now". Suppressed by `hideTimeOverride`. */}
      {badge && (
        <div className="lbdg-badge" aria-label={`time override for cell ${c.i}`}>
          {badge}
        </div>
      )}
      {/* Panel links — a titled URL list. External by default (arbitrary author URLs), so
          `rel="noreferrer"` and a new tab unless the author says otherwise. */}
      {links.length > 0 && (
        <div className="lbdg-no-drag lbdg-links">
          {links.map((l, i) => (
            <a
              key={`${l.url}-${i}`}
              href={l.url}
              title={l.title || l.url}
              aria-label={`panel link ${l.title || l.url}`}
              {...(l.targetBlank === false ? {} : { target: "_blank", rel: "noreferrer" })}
              className="lbdg-link"
            >
              <LinkIcon size={11} />
              <span className="lbdg-link-title">{l.title || l.url}</span>
            </a>
          ))}
        </div>
      )}
      {/* Edit affordances reveal on hover/focus (a polished board has a quiet resting state). */}
      {editable && (
        <button
          type="button"
          aria-label={`move cell ${c.i}`}
          title="Move widget"
          className="lbdg-drag-handle lbdg-btn lbdg-move"
        >
          <GripHorizontal size={13} />
        </button>
      )}
      {editable && (onEditPanel || onDuplicate || onExportCell || onRemove) && (
        <div className="lbdg-no-drag lbdg-chrome">
          {onEditPanel && (
            <button
              type="button"
              aria-label={`edit cell ${c.i}`}
              title="Edit panel"
              className="lbdg-btn"
              onClick={() => onEditPanel(c.i)}
            >
              <Pencil size={13} />
            </button>
          )}
          {onDuplicate && (
            <button
              type="button"
              aria-label={`duplicate cell ${c.i}`}
              title="Duplicate widget"
              className="lbdg-btn"
              onClick={() => onDuplicate(c.i)}
            >
              <Copy size={13} />
            </button>
          )}
          {onExportCell && (
            <button
              type="button"
              aria-label={`export cell ${c.i}`}
              title="Export widget"
              className="lbdg-btn"
              onClick={() => onExportCell(c.i)}
            >
              <Download size={13} />
            </button>
          )}
          {onRemove && (
            <button
              type="button"
              aria-label={`remove cell ${c.i}`}
              title="Remove widget"
              className="lbdg-btn lbdg-btn--danger"
              onClick={() => onRemove(c.i)}
            >
              <X size={13} />
            </button>
          )}
        </div>
      )}
      <div className="lbdg-cell-body">
        {Renderer ? (
          <Renderer cell={c} range={range} scope={scope} refreshKey={refreshKey} editable={editable} />
        ) : (
          <UnknownView view={view} />
        )}
      </div>
    </>
  );
}
