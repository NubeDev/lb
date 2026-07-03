// The library-panel wire shapes — mirror the gateway's `panel.*` routes + the host `Panel` record
// (library-panels scope). A panel is the reusable NON-LAYOUT half of a v3 dashboard cell (the spec):
// `view`/`title`/`sources[]`/`transformations`/`fieldConfig`/`options`/…, minus the grid geometry. It
// renders on many dashboards (as a ref cell) AND standalone on `/t/$ws/panel/{id}`. A panel is a LENS
// over data access — sharing it shares the DEFINITION; its `sources[]` re-check under the viewer's caps
// at render (a shared panel never widens data access).

import type { Cell } from "@/lib/dashboard";

/** The S4 asset-sharing visibility tiers (identical to a dashboard's). */
export type Visibility = "private" | "team" | "workspace";

/** The reusable panel definition — exactly the non-layout half of a v3 {@link Cell}. Every field
 *  mirrors the corresponding `Cell` field, so a spec is a `Cell` minus `i/x/y/w/h` (+ `panelRef`). */
export type PanelSpec = Omit<Cell, "i" | "x" | "y" | "w" | "h" | "panelRef" | "panelVars" | "panelMissing">;

/** A full panel record (the spec + sharing metadata). */
export interface Panel {
  id: string;
  title: string;
  owner: string;
  visibility: Visibility;
  spec: PanelSpec;
  schemaVersion?: number;
  updated_ts: number;
  deleted?: boolean;
}

/** The cheap roster row `panel.list` returns (no spec body; `view` for the picker icon). */
export interface PanelSummary {
  id: string;
  title: string;
  view: string;
  visibility: Visibility;
  updated_ts: number;
}

/** One dashboard that references a panel — the `panel.usage` row (delete-safety + editor banner). */
export interface PanelUsageRow {
  dashboard: string;
  title: string;
  cells: number;
}
