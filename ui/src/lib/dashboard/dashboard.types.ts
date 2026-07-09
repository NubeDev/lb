// The dashboard wire shapes — mirror the gateway's `dashboard.*` routes + the host `Dashboard`
// record (dashboard scope). A dashboard is a persisted grid of cells; each cell names a widget type
// and a data binding. The binding is the forever-contract Phase 2 moves behind the federation bridge
// unchanged: an explicit `series`, OR a tag-facet query resolved via the shipped `series.find`.

import type { Variable } from "@/lib/vars";
import type { FieldConfig } from "./fieldconfig.types";
export type { Variable };
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

/** The Phase-1 built-in widget types (v1). v2's render vocabulary is {@link View}. */
export type WidgetType = "chart" | "stat" | "gauge";

/** The v2/v3 render vocabulary. v2 (widget-builder scope): read views render a tool's result;
 *  scripted views run author code in a sandboxed iframe (and MAY write); control views call a write
 *  tool; `ext:<id>/<widget>` mounts an extension-shipped tile. v3 (viz chart-types scope) ADDS
 *  Grafana's panel-type ids as the canonical vocabulary (`timeseries`, `barchart`, `bargauge`,
 *  `piechart`, …); the shipped views remain valid ALIASES (`chart` → `timeseries`). New cells write
 *  the canonical id; old cells still read through {@link canonicalView}. */
export type View =
  // shipped v2 views (kept as aliases / escape-hatch views)
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
  // flow⇄dashboard binding UX: a validated JSON-payload control (write) + a JSON/object read view.
  | "json"
  | "jsonview"
  // v3 Grafana panel-type ids (the canonical vocabulary)
  | "timeseries"
  | "barchart"
  | "bargauge"
  | "piechart"
  // generative-UI: an AI-authored, catalog-constrained widget rendered from a persisted IR
  // (genui-scope). `options.genui = { v, ir, meta? }`; data flows through ordinary v3 `sources[]`.
  | "genui"
  // insights: a triage list of the workspace's insights (findings raised by rules/flows/agents),
  // rendered by the reusable `@nube/insights` widget. NOT source-bound — it reads the `insight.*`
  // verbs through the shell's `InsightsClient`, filtered by `options.insights`. Read-only by default;
  // `options.insights.interactive` turns on inline ack/resolve/dismiss (insights-package-scope).
  | "insights"
  // weather: a shadcn Card of current conditions from the host-native `weather.current` verb
  // (weather scope). Source-bound like any other read view; `options.{lat,lon}` names the location.
  | "weather"
  // panel-rows: a full-width, titled, collapsible section header that groups the cells positionally
  // beneath it (panel-rows scope). A LAYOUT view, not a data-bound viz — the grid special-cases it to
  // draw a header bar and fold/unfold its members; `options.collapsed` is the view flag. `w:24`, small
  // `h`. Membership is derived from geometry (see `rows.ts`), matching Grafana's expanded encoding.
  | "row"
  | `ext:${string}`;

/** A datasource reference (viz datasource-binding scope; Phase 1 is always native `surreal`). `uid`
 *  names a registered `datasource:{ws}:{name}` for federation; absent for native. */
export interface DataSourceRef {
  type: "surreal" | "series" | "federation" | string;
  uid?: string;
}

/** A Grafana "target" — one query against one datasource (viz panel-model scope). Generalizes the
 *  single {@link Source}; `refId` (A,B,…) is referenced by transformations + overrides. A v2
 *  single-`source` cell reads as `sources[0]` through {@link cellSources}. */
export interface Target {
  refId: string;
  datasource?: DataSourceRef;
  tool: string;
  args?: Record<string, unknown>;
  hide?: boolean;
}

/** Per-panel query options (Grafana's query-options row). All optional — absent means "use defaults".
 *  `maxDataPoints` soft-caps the returned point count; `minInterval` is a floor on the sampling step
 *  (e.g. `"1m"`); `relativeTime` overrides the dashboard range for this panel (e.g. `"now-6h"`). */
export interface QueryOptions {
  maxDataPoints?: number;
  minInterval?: string;
  relativeTime?: string;
}

/** A client-side transformation (viz transformations scope; shape only in Phase 1). */
export interface Transformation {
  id: string;
  options?: Record<string, unknown>;
  disabled?: boolean;
  filter?: import("./fieldconfig.types").Matcher;
}

/** The alias map (viz chart-types scope, "The alias map"): a shipped v2 `view` → its canonical
 *  Grafana panel-type id. A v2 `chart` cell IS a `timeseries` panel. Views not listed are their own
 *  canonical id (the Grafana ids, the scripted/control views, and `ext:` tiles pass through). */
const VIEW_ALIASES: Partial<Record<string, View>> = {
  chart: "timeseries",
};

/** Resolve a view to its canonical Grafana panel-type id (so `view:"chart"` and `view:"timeseries"`
 *  render identically). Non-aliased views (scripted/control/ext/already-canonical) pass through. */
export function canonicalView(view: View | string): View {
  return (VIEW_ALIASES[view] ?? view) as View;
}

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

  // --- v3 (viz panel-model scope) — all additive/optional; absent on a v1/v2 cell ---
  /** Panel description (Grafana parity). */
  description?: string;
  /** v3 targets — supersedes the single `source`. `sources[0]` === `source` for v2 compat (read
   *  through {@link cellSources}, which falls back to `source` when `sources` is absent). */
  sources?: Target[];
  /** The client-side transformation pipeline (shape only in Phase 1). */
  transformations?: Transformation[];
  /** Query options (Grafana's panel query options row): a soft cap on returned points, a minimum
   *  sampling interval, and a per-panel relative time window. Additive/optional; forwarded to the
   *  resolver with the whole panel. Absent on a cell that never set them (byte-clean round-trip). */
  queryOptions?: QueryOptions;
  /** Per-field option defaults + overrides (units/decimals/thresholds/mappings/color). The render
   *  bridge (`features/dashboard/fieldconfig/*`) formats values through it via user-prefs. */
  fieldConfig?: FieldConfig;
  /** Plugin version, for import/export round-trip fidelity. */
  pluginVersion?: string;

  // --- library-panels scope — a REF cell (additive; absent on an inline cell) ---
  /** When set (`panel:{id}`) this cell is a REF cell: it carries only layout + the ref + bounded
   *  per-placement overrides (`title`, `panelVars`) and NO spec — `dashboard.get` hydrates the spec
   *  from the panel record host-side, keeping this marker so the editor can offer link/unlink. */
  panelRef?: string;
  /** Per-placement variable bindings for a ref cell (the bounded override set). */
  panelVars?: Record<string, unknown>;
  /** Set by host hydration when a ref cannot be resolved (deleted/unshared/unreadable) — the cell
   *  renders the honest "panel not accessible" placeholder, never a leaked spec. Never persisted. */
  panelMissing?: boolean;
}

/** A cell's targets, v3 — `sources[]` when present, else the v2 single `source` as a one-element
 *  `[A]`, else `[]`. The ONE adapter that lets the whole render/edit path treat a v2 cell as a v3
 *  one-target cell (panel-model scope, Risks: "treat a single `source` as `sources[0]` everywhere"). */
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
 *  CANONICALIZED through the alias map so `chart`/`timeseries` render via the one timeseries renderer.
 *  `ext:`/scripted/control views pass through unchanged. A cell with NEITHER a view nor a widget_type
 *  (a malformed / half-authored cell) defaults to `timeseries` — the same default the editor's viz picker
 *  shows — so it renders a chart instead of the raw "unsupported view:" fallback. A real, non-empty but
 *  unknown view still falls through to the dispatcher's unsupported state (an honest "this view id isn't a
 *  thing"), which this default does NOT mask. */
export function cellView(cell: Cell): View {
  return canonicalView((cell.view as View) || (cell.widget_type as View) || "timeseries");
}

/** A cell's effective field-config, defaulted to empty (so a v1/v2 cell renders with no field options
 *  — today's behavior, the user-prefs fallback). The render bridge consumes this. */
export function cellFieldConfig(cell: Cell): FieldConfig {
  return cell.fieldConfig ?? { defaults: {}, overrides: [] };
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
  /** OUR panel-model document version (viz panel-model scope) — pinned at save, used by import/export
   *  + migration. Distinct from `Cell.v` (the cell contract version) and NOT Grafana's `schemaVersion`. */
  schemaVersion?: number;
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
