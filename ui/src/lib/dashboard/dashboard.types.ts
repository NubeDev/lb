// The dashboard wire shapes — mirror the gateway's `dashboard.*` routes + the host `Dashboard`
// record (dashboard scope). A dashboard is a persisted grid of cells; each cell names a widget type
// and a data binding. The binding is the forever-contract Phase 2 moves behind the federation bridge
// unchanged: an explicit `series`, OR a tag-facet query resolved via the shipped `series.find`.

/** The Phase-1 built-in widget types. Phase 2 adds `ext:<id>` (federated widgets). */
export type WidgetType = "chart" | "stat" | "gauge";

/** A cell's data binding: an explicit series name, OR a tag-facet query (resolved via `series.find`). */
export type Binding = { series: string } | { find: { tags: string[] } };

/** The S4 asset-sharing visibility tiers. */
export type Visibility = "private" | "team" | "workspace";

/** One grid cell — react-grid-layout geometry + the widget it hosts + its binding + options. */
export interface Cell {
  /** react-grid-layout item key (stable per cell). */
  i: string;
  x: number;
  y: number;
  w: number;
  h: number;
  widget_type: WidgetType;
  binding: Binding;
  /** Widget-type-specific options (unit label, thresholds, range). */
  options?: Record<string, unknown>;
}

/** A full dashboard record (the layout + sharing metadata). */
export interface Dashboard {
  id: string;
  title: string;
  owner: string;
  visibility: Visibility;
  cells: Cell[];
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
