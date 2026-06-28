// The dashboard wire shapes — mirror the gateway's `dashboard.*` routes + the host `Dashboard`
// record (dashboard scope). A dashboard is a persisted grid of cells; each cell names a widget type
// and a data binding. The binding is the forever-contract Phase 2 moves behind the federation bridge
// unchanged: an explicit `series`, OR a tag-facet query resolved via the shipped `series.find`.

import type { Variable } from "@/lib/vars";
export type { Variable };

/** The Phase-1 built-in widget types (v1). v2's render vocabulary is {@link View}. */
export type WidgetType = "chart" | "stat" | "gauge";

/** The v2 render vocabulary (widget-builder scope, "The widget taxonomy"). Read views render a
 *  tool's result; scripted views run author code in a sandboxed iframe (and MAY write); control views
 *  call a write tool; `ext:<id>/<widget>` mounts an extension-shipped tile. */
export type View =
  | "chart"
  | "stat"
  | "gauge"
  | "table"
  | "plot"
  | "d3"
  | "template"
  | "switch"
  | "slider"
  | "button"
  | `ext:${string}`;

/** A cell's data binding: an explicit series name, OR a tag-facet query (resolved via `series.find`). */
export type Binding = { series: string } | { find: { tags: string[] } };

/** A v2 cell source — ANY granted MCP tool call (read or write), re-checked at the host per call
 *  (widget-builder scope, "The widget contract, v2"). */
export interface Source {
  tool: string;
  args?: Record<string, unknown>;
}

/** A control's write action — the tool a switch/slider/button calls on interaction. `argsTemplate`
 *  carries one `{{value}}` slot the interaction fills (a typed template, open-Q4 lean). */
export interface Action {
  tool: string;
  argsTemplate?: Record<string, unknown>;
}

/** The S4 asset-sharing visibility tiers. */
export type Visibility = "private" | "team" | "workspace";

/** One grid cell — react-grid-layout geometry + the widget it hosts + its binding/source + options.
 *
 *  v1 cells carry `widget_type` + `binding`. v2 cells carry `v:2`, `view`, `source`, and (for
 *  controls) `action`. All v2 fields are optional so a v1 series cell stays valid; a renderer reads
 *  `view` when present, else falls back to `widget_type` (a v1 cell is a v2 cell whose tool set is the
 *  four series read verbs). */
export interface Cell {
  /** react-grid-layout item key (stable per cell). */
  i: string;
  x: number;
  y: number;
  w: number;
  h: number;
  /** Contract version. Absent/0/1 = a v1 series cell; 2 = a v2 tool-bound cell. */
  v?: number;
  widget_type: WidgetType;
  /** A human title for the cell (widget-config-vars scope, Slice 1). Optional/additive; the header
   *  renders it, falling back to a derived label when empty. Persisted via `dashboard.save`. */
  title?: string;
  /** v2 render vocabulary. Empty on a v1 cell. */
  view?: View;
  /** v1 binding (kept for v1 compatibility). */
  binding: Binding;
  /** v2 source: the `{ tool, args }` the cell reads/streams. */
  source?: Source;
  /** v2 action: a control's write `{ tool, argsTemplate }`. */
  action?: Action;
  /** Widget-type-specific options (unit label, thresholds, range, inline template code). */
  options?: Record<string, unknown>;
}

/** Resolve a cell's effective render view — `view` (v2) when present, else `widget_type` (v1). */
export function cellView(cell: Cell): View {
  return (cell.view as View) || (cell.widget_type as View);
}

/** A cell's header label: the author `title` when set, else a derived fallback — the source tool, an
 *  ext-tile name, or the view (widget-config-vars scope, Slice 1: "Header renders title (fallback to
 *  derived label)"). Never the empty string, so the header always reads something honest. */
export function cellLabel(cell: Cell): string {
  if (cell.title?.trim()) return cell.title.trim();
  if (cell.source?.tool) return cell.source.tool;
  if (cell.action?.tool) return cell.action.tool;
  const v = cellView(cell);
  return v || cell.widget_type || "widget";
}

/** A full dashboard record (the layout + sharing metadata + variable definitions). */
export interface Dashboard {
  id: string;
  title: string;
  owner: string;
  visibility: Visibility;
  cells: Cell[];
  /** Variable definitions (widget-config-vars Slice 2). The per-viewer selection lives in the URL. */
  variables?: Variable[];
  updated_ts: number;
  deleted?: boolean;
}

/** The cheap roster row `list` returns (no cell bodies). */
export interface DashboardSummary {
  id: string;
  title: string;
  visibility: Visibility;
  updated_ts: number;
}

/** Narrow a binding to its explicit series name, if it has one (vs a tag-facet query). */
export function bindingSeries(binding: Binding): string | null {
  return "series" in binding ? binding.series : null;
}

/** The tag strings (`key:value`) of a tag-facet binding, or `[]` for a series binding. */
export function bindingTags(binding: Binding): string[] {
  return "find" in binding ? binding.find.tags : [];
}
