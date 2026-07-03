// The Panel↔Cell bridge (library-panels scope). A panel spec IS the non-layout half of a v3 cell, so
// converting between them is dropping/adding the grid geometry — no parallel model, no re-mapping. Used
// by: the standalone page (panel → a renderable cell), Save-as-library (cell → spec + a ref cell), and
// Unlink (a hydrated ref cell → an inline cell). One responsibility: the two directions of that split.

import type { Cell } from "@/lib/dashboard";
import type { PanelSpec } from "./panel.types";

/** Extract a panel spec from a cell — everything EXCEPT the per-dashboard placement + ref marker. This
 *  is the "Save as library panel" payload. */
export function cellToSpec(cell: Cell): PanelSpec {
  const { i: _i, x: _x, y: _y, w: _w, h: _h, panelRef: _r, panelVars: _v, panelMissing: _m, ...spec } = cell;
  return spec;
}

/** Build a renderable cell from a panel spec + a chosen grid geometry. The standalone page renders ONE
 *  full-bleed cell; the grid gives a ref cell its layout. `i` defaults to the panel id. */
export function specToCell(id: string, spec: PanelSpec, layout?: Partial<Pick<Cell, "x" | "y" | "w" | "h">>): Cell {
  return {
    i: id,
    x: layout?.x ?? 0,
    y: layout?.y ?? 0,
    w: layout?.w ?? 12,
    h: layout?.h ?? 8,
    ...spec,
  };
}

/** A ref cell: layout + the `panel:{id}` marker, no spec (the host hydrates it). Save-as-library turns
 *  the authored cell into this. */
export function refCell(cell: Cell, panelId: string): Cell {
  return {
    i: cell.i,
    x: cell.x,
    y: cell.y,
    w: cell.w,
    h: cell.h,
    widget_type: cell.widget_type,
    binding: cell.binding,
    // Title override + var bindings are the bounded per-placement overrides a ref keeps.
    title: cell.title,
    panelRef: panelId.startsWith("panel:") ? panelId : `panel:${panelId}`,
    panelVars: cell.panelVars,
  };
}

/** Unlink: copy the (hydrated) spec back inline and drop the ref — drift becomes explicit and the
 *  caller's own (library-panels scope, the explicit "stop tracking" act). */
export function unlinkCell(hydrated: Cell): Cell {
  const { panelRef: _r, panelVars: _v, panelMissing: _m, ...inline } = hydrated;
  return inline;
}
