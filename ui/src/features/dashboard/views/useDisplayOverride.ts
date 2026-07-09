// A reusable display-only VIEW override for a rendered cell (dashboard scope). It is the same trick the
// editor's `PreviewPane`/`WizardPreview` use to sanity-check data behind a draft — clone the cell with a
// swapped `view` so the SAME `WidgetView` dispatch draws the frames as a `table` or pretty-prints them
// through `jsonview`, WITHOUT touching the saved cell. Here it powers a per-widget toggle icon on a live
// board: one icon that cycles viz → table → JSON → viz. View-only; never persisted.
//
// One responsibility: own the override state + the cloned cell + the toggle's icon/label. The Grid (or
// any host) renders one button off `cycle`/`icon`/`title` and passes `applyTo(cell)` to `WidgetHost`.

import { useState } from "react";
import { BarChart3, Braces, Table2, type LucideIcon } from "lucide-react";

import type { Cell } from "@/lib/dashboard";
import { cellView } from "@/lib/dashboard";

/** null = draw the cell's own viz. Otherwise a display-only `view` override. */
export type DisplayOverride = null | "table" | "jsonview";

/** The views for which a table/JSON inspect is meaningful — the read views that resolve real frames.
 *  Control views (switch/slider/button/json write control), layout rows, and the insights triage list
 *  are NOT source-frame reads, so the toggle is hidden for them (nothing to tabulate/serialize). */
const READ_VIEWS = new Set([
  "timeseries",
  "chart",
  "stat",
  "gauge",
  "bargauge",
  "table",
  "barchart",
  "piechart",
  "weather",
]);

/** True when this cell renders resolvable frames a table/JSON view can show. */
export function canInspect(cell: Cell): boolean {
  const view = cellView(cell);
  if (view.startsWith("ext:")) return false;
  return READ_VIEWS.has(view);
}

const NEXT: Record<string, DisplayOverride> = { none: "table", table: "jsonview", jsonview: null };
const META: Record<string, { icon: LucideIcon; title: string }> = {
  none: { icon: BarChart3, title: "Showing chart — click for table" },
  table: { icon: Table2, title: "Showing table — click for JSON" },
  jsonview: { icon: Braces, title: "Showing JSON — click for chart" },
};

/** Per-cell display override. `override` is the current mode; `cycle` advances viz → table → JSON → viz;
 *  `applyTo` returns the cell to render (cloned with the swapped view when overridden). */
export function useDisplayOverride() {
  const [override, setOverride] = useState<DisplayOverride>(null);
  const key = override ?? "none";
  return {
    override,
    cycle: () => setOverride((o) => NEXT[o ?? "none"]),
    icon: META[key].icon,
    title: META[key].title,
    /** The cell to hand to `WidgetView` — the saved cell untouched, only its drawn `view` swapped. */
    applyTo: (cell: Cell): Cell => (override ? { ...cell, view: override } : cell),
  };
}
