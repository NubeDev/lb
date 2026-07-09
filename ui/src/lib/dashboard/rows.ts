// Panel-rows membership + collapse geometry (panel-rows scope). A row is a `Cell{ view:"row" }` living
// in the same flat `cells[]`; membership is POSITIONAL — the cells whose `y` falls between this row's
// `y` and the next row's `y` belong to it (Grafana's expanded encoding). No `rowId` on child cells; we
// derive it from geometry. `options.collapsed` is a render-time flag: the layout hook zeroes members'
// effective height so the grid pulls the space closed, keeping their real `x/y/w/h` in the record so
// expand restores them exactly. This is the ONE place that knows "the cells under a row are the ones
// between its y and the next row's y" (panel-rows scope, "Intent / approach").

import type { Cell } from "./dashboard.types";
import { cellView } from "./dashboard.types";

/** The full width a row header spans — our grid is 12 columns (Grid.tsx `COLS`), so a row is 12 wide. */
export const ROW_W = 12;
/** A row header's height in grid units — a short bar, not a widget frame. */
export const ROW_H = 1;

/** Is this cell a row header? */
export function isRow(cell: Cell): boolean {
  return cellView(cell) === "row";
}

/** Is this row collapsed? (`options.collapsed === true`) — this doubles as the row's DEFAULT open/closed
 *  state: it's the stored collapse flag applied on load (panel-rows options). */
export function isCollapsed(cell: Cell): boolean {
  return isRow(cell) && cell.options?.collapsed === true;
}

/** A row header's presentation options, defaulted (panel-rows options). `showCount` = show the "· N
 *  panels" member count beside the title; `showLine` = draw the bottom divider line; `collapsed` = the
 *  stored default open/closed state. Both display flags default TRUE (today's look) so a pre-options row
 *  is unchanged; only an explicit `false` hides them. */
export interface RowOptions {
  showCount: boolean;
  showLine: boolean;
  collapsed: boolean;
}

/** Read a row cell's presentation options with defaults. A non-row cell reads as all-default. */
export function rowOptions(cell: Cell): RowOptions {
  const o = cell.options ?? {};
  return {
    showCount: o.showCount !== false,
    showLine: o.showLine !== false,
    collapsed: o.collapsed === true,
  };
}

/** The row cells in a dashboard, ordered by `y` (then `x`) — the section boundaries. */
export function rows(cells: Cell[]): Cell[] {
  return cells.filter(isRow).sort((a, b) => a.y - b.y || a.x - b.x);
}

/** The member cells of `row` — every NON-row cell whose `y` is ≥ the row's `y` and < the next row's `y`
 *  (positional membership). The next row is the row with the smallest `y` strictly greater than this
 *  one's; a trailing row owns everything below it. A row is never its own member. If `row` is not
 *  actually a row cell in `cells`, returns `[]` (a defensive no-op). */
export function rowMembers(cells: Cell[], row: Cell): Cell[] {
  if (!isRow(row)) return [];
  const rs = rows(cells);
  const idx = rs.findIndex((r) => r.i === row.i);
  if (idx < 0) return [];
  const start = row.y;
  const next = rs[idx + 1];
  const end = next ? next.y : Number.POSITIVE_INFINITY;
  return cells.filter((c) => !isRow(c) && c.y >= start && c.y < end);
}

/** The cells that belong to NO row — those with a `y` above the first row (the ungrouped top-of-board
 *  region). A dashboard with no rows returns every non-row cell here. */
export function ungroupedCells(cells: Cell[]): Cell[] {
  const rs = rows(cells);
  const firstRowY = rs.length > 0 ? rs[0].y : Number.POSITIVE_INFINITY;
  return cells.filter((c) => !isRow(c) && c.y < firstRowY);
}

/** The cells the grid should actually render, with collapsed rows' members hidden. A collapsed row's
 *  members are DROPPED from the render list (kept in the record); the row header itself always renders.
 *  This is the render-time transform (panel-rows scope, "collapse is a render-time transform") — it
 *  never mutates the stored geometry. Non-row cells and expanded rows pass through unchanged. */
export function visibleCells(cells: Cell[]): Cell[] {
  const collapsed = rows(cells).filter(isCollapsed);
  if (collapsed.length === 0) return cells;
  const hidden = new Set<string>();
  for (const row of collapsed) {
    for (const m of rowMembers(cells, row)) hidden.add(m.i);
  }
  return cells.filter((c) => !hidden.has(c.i));
}
