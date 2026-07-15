// The drag/resize-stop merge — react-grid-layout's new geometry folded back onto the cells
// (which carry binding/options/type), PURE so it is directly testable. A row that MOVED carries
// its members: we compute the row's Δy (new `y` − old `y`) and shift every cell that was a
// positional member of that row BEFORE the move by the same Δy — so a section stays intact when
// its header is dragged (panel-rows: "dragging a row must carry its members"). Members are
// resolved against the pre-move `cells`; a collapsed row's members are hidden from the layout
// but still shift, keeping the section contiguous on expand.

import type { Cell } from "./dashboard.types";
import { isRow, rowMembers } from "./rows";

/** The slice of a react-grid-layout item the merge reads (avoids importing RGL types here). */
export interface LayoutItem {
  i: string;
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Merge a new layout (geometry only) back onto `cells`. Cells present in `next` take its
 *  geometry verbatim (the layout is authoritative for on-screen items); hidden members of a
 *  moved row shift by the row's Δy; everything else passes through unchanged. */
export function mergeLayout(cells: Cell[], next: LayoutItem[]): Cell[] {
  const byKey = new Map(next.map((l) => [l.i, l]));
  // Δy per moved row, applied to the member keys the layout did NOT touch (hidden/collapsed).
  const memberShift = new Map<string, number>();
  for (const c of cells) {
    if (!isRow(c)) continue;
    const l = byKey.get(c.i);
    if (!l) continue;
    const dy = l.y - c.y;
    if (dy === 0) continue;
    for (const m of rowMembers(cells, c)) {
      if (!byKey.has(m.i)) memberShift.set(m.i, dy);
    }
  }
  return cells.map((c) => {
    const l = byKey.get(c.i);
    if (l) return { ...c, x: l.x, y: l.y, w: l.w, h: l.h };
    const dy = memberShift.get(c.i);
    return dy ? { ...c, y: c.y + dy } : c;
  });
}
