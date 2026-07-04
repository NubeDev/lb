// The grid host — a `react-grid-layout` of widget cells (dashboard scope). The layout maps 1:1 to the
// dashboard's `cells[]` record; drag/resize stops persist the new geometry via `onLayout` (→
// `dashboard.save`, the SurrealDB record — never localStorage). A measured width (with a sane
// fallback) keeps it deterministic in tests. Read-only viewers get a non-draggable grid.

import { useEffect, useRef, useState } from "react";
import GridLayout, { type Layout } from "react-grid-layout";
import { GripHorizontal, X } from "lucide-react";

import { WidgetHost } from "./WidgetHost";
import type { Cell } from "@/lib/dashboard";
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

  const layout: Layout[] = cells.map((c) => ({ i: c.i, x: c.x, y: c.y, w: c.w, h: c.h }));

  // Merge a new layout (geometry only) back onto the cells (which carry binding/options/type).
  const apply = (next: Layout[]) => {
    const byKey = new Map(next.map((l) => [l.i, l]));
    onLayout(
      cells.map((c) => {
        const l = byKey.get(c.i);
        return l ? { ...c, x: l.x, y: l.y, w: l.w, h: l.h } : c;
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
        {cells.map((c) => (
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
              <button
                aria-label={`remove cell ${c.i}`}
                title="Remove widget"
                className="widget-no-drag absolute right-2 top-2 z-10 inline-flex h-6 w-6 items-center justify-center rounded-md text-muted opacity-0 transition-[opacity,color,background-color] hover:bg-destructive/12 hover:text-destructive focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-destructive/30 group-hover/cell:opacity-100"
                onClick={() => onRemove(c.i)}
              >
                <X size={13} />
              </button>
            )}
            <div className="min-h-0 flex-1 p-3">
              <WidgetHost cell={c} range={range} installed={installed} workspace={workspace} scope={scope} refreshKey={refreshKey} />
            </div>
          </div>
        ))}
      </GridLayout>
    </div>
  );
}
