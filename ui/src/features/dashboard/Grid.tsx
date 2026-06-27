// The grid host — a `react-grid-layout` of widget cells (dashboard scope). The layout maps 1:1 to the
// dashboard's `cells[]` record; drag/resize stops persist the new geometry via `onLayout` (→
// `dashboard.save`, the SurrealDB record — never localStorage). A fixed `width` (measured, with a
// sane fallback) keeps it deterministic in tests. Read-only viewers get a non-draggable grid.

import { useEffect, useRef, useState } from "react";
import GridLayout, { type Layout } from "react-grid-layout";
import { X } from "lucide-react";

import { WidgetHost } from "./WidgetHost";
import type { Cell } from "@/lib/dashboard";

// react-grid-layout's stylesheet (the resize-handle styles it bundles from react-resizable are
// included here; react-resizable is a transitive dep, not directly resolvable under pnpm).
import "react-grid-layout/css/styles.css";

interface Props {
  cells: Cell[];
  editable: boolean;
  /** Called with the new cell geometry on a drag/resize stop (the persistence seam). */
  onLayout: (cells: Cell[]) => void;
  onRemove: (i: string) => void;
}

const COLS = 12;
const ROW_H = 56;

export function Grid({ cells, editable, onLayout, onRemove }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(1200);

  useEffect(() => {
    const measure = () => {
      const w = ref.current?.offsetWidth ?? 0;
      if (w > 0) setWidth(w);
    };
    measure();
    window.addEventListener("resize", measure);
    return () => window.removeEventListener("resize", measure);
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
    <div ref={ref} className="h-full overflow-auto" aria-label="dashboard grid">
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
        draggableCancel=".widget-no-drag"
      >
        {cells.map((c) => (
          <div
            key={c.i}
            className="flex flex-col rounded-md border border-border bg-panel p-2"
            aria-label={`cell ${c.i}`}
          >
            {editable && (
              <button
                aria-label={`remove cell ${c.i}`}
                title="Remove widget"
                className="widget-no-drag absolute right-1 top-1 z-10 rounded p-0.5 text-muted hover:text-red-400"
                onClick={() => onRemove(c.i)}
              >
                <X size={12} />
              </button>
            )}
            <div className="min-h-0 flex-1">
              <WidgetHost cell={c} />
            </div>
          </div>
        ))}
      </GridLayout>
    </div>
  );
}
