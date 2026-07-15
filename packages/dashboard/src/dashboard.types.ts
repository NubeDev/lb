// The dashboard cell vocabulary — the PURE model of a persisted grid of cells (@nube/dashboard).
// Ported from the shell's `lib/dashboard/dashboard.types.ts` with the shell entanglements cut:
// no `@/lib/vars` Variable import (a dashboard record's `variables` is opaque `unknown[]` here —
// the variables machinery is the consumer's), no transport. A cell names a render view, a data
// binding/sources, and react-grid-layout geometry; the package renders geometry + dispatches the
// view through the consumer's widget registry. It NEVER fetches, persists, or knows a workspace.

import type { DataLink, FieldConfig } from "./fieldconfig.types";
export type {
  FieldConfig,
  FieldOptions,
  FieldOverride,
  Matcher,
  ValueMapping,
  ValueMappingResult,
  ThresholdsConfig,
  FieldColor,
  FieldColorModeId,
  DataLink,
} from "./fieldconfig.types";

/** The grid's column count — the geometry vocabulary every cell's `x`/`w` is written against. */
export const GRID_COLS = 12;
/** One grid row's pixel height (react-grid-layout `rowHeight`). */
export const GRID_ROW_PX = 56;

/** The Phase-1 built-in widget types (v1). v2's render vocabulary is {@link View}. */
export type WidgetType = "chart" | "stat" | "gauge";

/** The v2/v3 render vocabulary. Read views render a tool's result; scripted views run author
 *  code; control views call a write tool; `ext:<id>/<widget>` mounts an extension-shipped tile.
 *  v3 ADDS Grafana's panel-type ids as the canonical vocabulary (`timeseries`, `barchart`, …);
 *  the shipped v2 views remain valid ALIASES (`chart` → `timeseries`) via {@link canonicalView}.
 *  The PACKAGE ships no renderer for any of these — the consumer's registry does; the vocabulary
 *  lives here so every surface spells a view the same way. */
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
  | "json"
  | "jsonview"
  | "timeseries"
  | "barchart"
  | "bargauge"
  | "piechart"
  | "genui"
  | "insights"
  | "weather"
  | "text"
  // panel-rows: a full-width, titled, collapsible section header grouping the cells positionally
  // beneath it. A LAYOUT view — the grid special-cases it (see `rows.ts`); `options.collapsed`.
  | "row"
  | `ext:${string}`;

/** A datasource reference. `uid` names a registered datasource for federation; absent = native. */
export interface DataSourceRef {
  type: "surreal" | "series" | "federation" | string;
  uid?: string;
}

/** A Grafana "target" — one query against one datasource. Generalizes the single {@link Source};
 *  `refId` (A,B,…) is referenced by transformations + overrides. A v2 single-`source` cell reads
 *  as `sources[0]` through {@link cellSources}. */
export interface Target {
  refId: string;
  datasource?: DataSourceRef;
  tool: string;
  args?: Record<string, unknown>;
  hide?: boolean;
}

/** Per-panel query options (Grafana's query-options row). All optional — absent = defaults.
 *  The time-override fields are Grafana-verbatim; the host interprets them when dispatching
 *  targets — the grid only renders the badge. `hideTimeOverride` is display-only. */
export interface QueryOptions {
  maxDataPoints?: number;
  minInterval?: string;
  relativeTime?: string;
  /** Replaces the range with `[now - timeFrom, now]` for this panel (e.g. `"6h"`). */
  timeFrom?: string;
  /** Moves BOTH range ends earlier by this duration (e.g. `"1d"`) — a comparison offset. */
  timeShift?: string;
  /** Display-only: hide the override badge in the panel header. Never affects the query. */
  hideTimeOverride?: boolean;
}

/** A client-side transformation (shape only — applying it is the consumer's). */
export interface Transformation {
  id: string;
  options?: Record<string, unknown>;
  disabled?: boolean;
  filter?: import("./fieldconfig.types").Matcher;
}

/** The alias map: a shipped v2 `view` → its canonical Grafana panel-type id. A v2 `chart` cell
 *  IS a `timeseries` panel. Views not listed are their own canonical id. */
const VIEW_ALIASES: Partial<Record<string, View>> = {
  chart: "timeseries",
};

/** Resolve a view to its canonical panel-type id (so `view:"chart"` and `view:"timeseries"`
 *  dispatch to the one registered renderer). Non-aliased views pass through. */
export function canonicalView(view: View | string): View {
  return (VIEW_ALIASES[view] ?? view) as View;
}

/** A cell's data binding: an explicit series name, OR a tag-facet query. */
export type Binding = { series: string } | { find: { tags: string[] } };

/** A v2 cell source — ANY MCP tool call (read or write); the HOST re-checks the grant per call. */
export interface Source {
  tool: string;
  args?: Record<string, unknown>;
}

/** A control's write action — the tool a switch/slider/button calls on interaction. */
export interface Action {
  tool: string;
  argsTemplate?: Record<string, unknown>;
}

/** Asset-sharing visibility tiers. */
export type Visibility = "private" | "team" | "workspace";

/** Header-chrome visibility flags (each optional toolbar control is HIDDEN by default). */
export interface Toolbar {
  dateSelect?: boolean;
  refreshRate?: boolean;
  share?: boolean;
}

/** One grid cell — react-grid-layout geometry + the widget it hosts + its binding/source +
 *  options. v1 cells carry `widget_type` + `binding`; v2 add `v:2`, `view`, `source`, `action`;
 *  v3 fields are all additive/optional. A renderer reads `view` when present, else falls back to
 *  `widget_type` (see {@link cellView}). */
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
  /** A human title for the cell; the header renders it, falling back to a derived label. */
  title?: string;
  /** v2 render vocabulary. Empty on a v1 cell. */
  view?: View;
  /** v1 binding (kept for v1 compatibility). */
  binding: Binding;
  /** v2 source: the `{ tool, args }` the cell reads/streams. */
  source?: Source;
  /** v2 action: a control's write `{ tool, argsTemplate }`. */
  action?: Action;
  /** Widget-type-specific options (unit label, thresholds, range, inline template code…). */
  options?: Record<string, unknown>;

  // --- v3 (panel-model) — all additive/optional; absent on a v1/v2 cell ---
  description?: string;
  /** v3 targets — supersedes the single `source` (read through {@link cellSources}). */
  sources?: Target[];
  transformations?: Transformation[];
  queryOptions?: QueryOptions;
  /** Per-field option defaults + overrides. CARRIED as data in v0.1 — the apply/format bridge
   *  is NOT in this package; a consumer's renderers interpret it. */
  fieldConfig?: FieldConfig;
  pluginVersion?: string;

  /** Grafana's panel `transparent`: drop the frame chrome and sit directly on the board. */
  transparent?: boolean;
  /** Grafana panel `links` — a titled URL list shown in the panel header. */
  links?: DataLink[];

  // --- library-panels ref fields — ride along as opaque data (hydration is host-side) ---
  panelRef?: string;
  panelVars?: Record<string, unknown>;
  /** Set by host hydration when a ref can't resolve — renderers show an honest placeholder. */
  panelMissing?: boolean;
}

/** A cell's targets, v3 — `sources[]` when present, else the v2 single `source` as `[A]`, else
 *  `[]`. The ONE adapter that lets a renderer treat a v2 cell as a v3 one-target cell. */
export function cellSources(cell: Cell): Target[] {
  if (cell.sources && cell.sources.length > 0) return cell.sources;
  if (cell.source?.tool) {
    return [{ refId: "A", tool: cell.source.tool, args: cell.source.args, datasource: { type: "surreal" } }];
  }
  return [];
}

/** A cell's primary (first non-hidden) target — what a single-source view reads. */
export function cellPrimaryTarget(cell: Cell): Target | undefined {
  return cellSources(cell).find((t) => !t.hide) ?? cellSources(cell)[0];
}

/** Resolve a cell's effective render view — `view` (v2) when present, else `widget_type` (v1) —
 *  CANONICALIZED through the alias map. A cell with NEITHER (malformed / half-authored) defaults
 *  to `timeseries`; a real-but-unknown view still reaches the registry's honest unknown state. */
export function cellView(cell: Cell): View {
  return canonicalView((cell.view as View) || (cell.widget_type as View) || "timeseries");
}

/** A cell's effective field-config, defaulted to empty. */
export function cellFieldConfig(cell: Cell): FieldConfig {
  return cell.fieldConfig ?? { defaults: {}, overrides: [] };
}

/** A cell's header label: the author `title` when set, else a derived fallback — the source
 *  tool, an action tool, or the view. Never the empty string. */
export function cellLabel(cell: Cell): string {
  if (cell.title?.trim()) return cell.title.trim();
  if (cell.source?.tool) return cell.source.tool;
  if (cell.action?.tool) return cell.action.tool;
  const v = cellView(cell);
  return v || cell.widget_type || "widget";
}

/** A full dashboard record (layout + sharing metadata). `variables` is OPAQUE here — the
 *  variables machinery (definitions, URL selection, interpolation) is the consumer's. */
export interface Dashboard {
  id: string;
  title: string;
  description?: string;
  icon?: string;
  color?: string;
  toolbar?: Toolbar;
  timezone?: string;
  owner: string;
  visibility: Visibility;
  cells: Cell[];
  /** Variable definitions — opaque to the package (see the module comment). */
  variables?: unknown[];
  schemaVersion?: number;
  updated_ts: number;
  deleted?: boolean;
}

/** The cheap roster row a `list` returns (no cell bodies). */
export interface DashboardSummary {
  id: string;
  title: string;
  icon?: string;
  color?: string;
  visibility: Visibility;
  updated_ts: number;
}

/** Narrow a binding to its explicit series name, if it has one. */
export function bindingSeries(binding: Binding): string | null {
  return "series" in binding ? binding.series : null;
}

/** The tag strings (`key:value`) of a tag-facet binding, or `[]` for a series binding. */
export function bindingTags(binding: Binding): string[] {
  return "find" in binding ? binding.find.tags : [];
}
