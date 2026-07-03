// A fresh, default v3 cell for a given viz — what ADD seeds the editor with (viz panel-editor scope:
// "Add = cellToEditorState(defaultCell(view))"). A default cell is a real v3 cell: the canonical view,
// Grafana's per-viz option defaults, an empty single target, and an empty field-config — so ADD and
// EDIT enter the SAME (de)serializer with the SAME complete option surface. One responsibility: build
// the starting cell.

import type { Cell, View } from "@/lib/dashboard";
import { canonicalView } from "@/lib/dashboard";

/** The v3 contract version a freshly-authored cell carries. */
const CELL_V3 = 3;

/** A fresh default cell for `view` at grid key `i` with geometry `geom`. `options` is the per-view
 *  Grafana default option block — INJECTED by the caller (the view substrate's `defaultOptionsForView`
 *  registry owns "what a fresh <view>'s options look like"; panel-kit stays headless of the views). */
export function defaultCell(
  view: View,
  i: string,
  geom = { x: 0, y: 0, w: 8, h: 4 },
  options: Record<string, unknown> = {},
): Cell {
  const v = canonicalView(view);
  return {
    i,
    x: geom.x,
    y: geom.y,
    w: geom.w,
    h: geom.h,
    v: CELL_V3,
    // `widget_type` keeps a harmless v1 fallback (chart) — `view` is authoritative for v2/v3 cells.
    widget_type: "chart",
    view: v,
    binding: { series: "" },
    sources: [{ refId: "A", tool: "", args: {}, datasource: { type: "surreal" } }],
    options,
    fieldConfig: { defaults: {}, overrides: [] },
    // No `transformations` key on a fresh cell — it's born backend in Phase 3 (invariant B); the editor
    // state defaults it to `[]` and serialize omits an empty list, so the round-trip stays identity.
  };
}
