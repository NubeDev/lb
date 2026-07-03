// THE pure cell ↔ editor-state (de)serializer (viz panel-editor scope, "The fix"). This is the whole
// ballgame: ONE function pair that ADD and EDIT both use, so the option surface and the round-trip are
// provably identical and the user's "edit loses my SQL options / add ≠ edit" bug becomes impossible.
//
//   cellToEditorState(cell)  → EditorState   — rebuild EVERY group from the cell (viz, targets incl.
//                                              SQL Builder raw+builder query, per-view options,
//                                              fieldConfig defaults+overrides, transformations config,
//                                              title/description).
//   editorStateToCell(state, base) → Cell    — serialize back, PRESERVING the cell key + geometry +
//                                              v1 fields, emitting the additive v3 shape.
//
// The contract pinned by a unit test: `editorStateToCell(cellToEditorState(c), c) ≡ c` for v1/v2/v3
// cells. To make that hold losslessly, the editor state CARRIES the immutable parts (key/geometry/
// widget_type/binding/v) verbatim and only re-derives the editable groups; absent/empty groups stay
// absent so a v1/v2 cell round-trips byte-for-byte (no spurious `fieldConfig:{}` appears on a v2 cell).
//
// One responsibility: the (de)serialization. No React, no I/O, no rendering — a pure module so the
// round-trip test needs no gateway.

import type {
  Cell,
  FieldConfig,
  QueryOptions,
  Target,
  Transformation,
  View,
} from "@/lib/dashboard";
import { cellSources } from "@/lib/dashboard";
import type { SqlSourceState } from "./sql/query";

/** The full editor working state — every authorable group of the panel model, reconstructed from a
 *  cell and serialized back without loss. */
export interface EditorState {
  /** The cell's view, kept RAW (not canonicalized) so the round-trip is byte-identity: a v2 `chart`
   *  cell stays `chart` (rendered via the alias), an empty view stays empty (a v1 cell). The picker +
   *  per-view tabs compare via `canonicalView(state.view)`. A viz SWITCH writes the canonical id. */
  view: View | "";
  title: string;
  description: string;
  /** The targets (v3 `sources[]`; a v2 single-source cell reads as one target via `cellSources`). */
  targets: Target[];
  /** The SQL Builder state, when the primary target is a `store.query` source authored in the builder
   *  (rehydrated from `cell.options.sql` — the precise thing edit used to drop). */
  sql?: SqlSourceState;
  /** Per-view options (legend/tooltip/…) — opaque here; the option tabs own the typed shape. */
  options: Record<string, unknown>;
  /** The field-config (defaults + overrides). Undefined when the cell had none (so it stays absent). */
  fieldConfig?: FieldConfig;
  /** The transformation pipeline CONFIG (shape only in Phase 1; no client execution — invariant B). */
  transformations: Transformation[];
  /** Per-panel query options (max data points / min interval / relative time). Undefined when the cell
   *  had none (so it stays absent — byte-clean round-trip). */
  queryOptions?: QueryOptions;
  /** The opaque carry-over the editor doesn't model field-by-field but must round-trip (v1 binding,
   *  v2 action, scripted `code`/`templateId`, the v contract number, widget_type). Preserved verbatim. */
  carry: {
    v: number | undefined;
    widget_type: Cell["widget_type"];
    binding: Cell["binding"];
    action: Cell["action"];
    pluginVersion: string | undefined;
    /** How the cell encoded its targets, so serialize reproduces the SAME shape from `state.targets`:
     *  `"none"` (no source — a v1 series cell or a scripted/control cell), `"source"` (the v2 single
     *  `source`), or `"sources"` (the v3 `sources[]`). A v2 cell stays single-`source` (byte-identical
     *  round-trip); a target EDIT in the Query tab writes back through the same shape. */
    targetRepr: "none" | "source" | "sources";
    /** `options` keys the editor doesn't own (scripted `code`, `templateId`, `unit`, `min`, `max`, …)
     *  — kept so a v2 scripted/stat/gauge cell round-trips while a timeseries cell uses typed groups. */
    extraOptions: Record<string, unknown>;
  };
}

/** Option keys the editor models as typed per-view groups (so they're NOT duplicated into `carry`).
 *  Phase 1: legend/tooltip (timeseries) + sql. Phase 2 adds the single-stat + table + barchart per-viz
 *  option keys (Grafana names verbatim) so a fully-populated Phase-2 cell round-trips through the typed
 *  groups, not `extraOptions`. A stale v2 key (`unit`/`min`/`max`) stays in `extraOptions` (carry). */
const OWNED_OPTION_KEYS = new Set([
  "sql",
  // the genui widget's persisted IR + authoring meta (`{ v, ir, meta? }`) — owned so the AI-widget
  // author tab's write round-trips through the typed group, not `extraOptions`.
  "genui",
  // the shared X/Y plot spec (viz x/y builder) — a `PlotSpec` picking axes + chart type, rendered by
  // the shared `PlotChart` for the cartesian chart views. Owned so it round-trips through the editor.
  "plot",
  // timeseries / barchart / piechart shared
  "legend",
  "tooltip",
  // single-stat family (stat/gauge/bargauge/piechart) frame→value bridge
  "reduceOptions",
  // stat
  "graphMode",
  "colorMode",
  "justifyMode",
  "textMode",
  "showPercentChange",
  // gauge / bargauge
  "orientation",
  "showThresholdLabels",
  "showThresholdMarkers",
  "sizing",
  "minVizWidth",
  "minVizHeight",
  "displayMode",
  "valueMode",
  "showUnfilled",
  // barchart
  "stacking",
  "showValue",
  "barWidth",
  "groupWidth",
  "xTickLabelRotation",
  "xField",
  "colorByField",
  // piechart
  "pieType",
  "displayLabels",
  // table
  "showHeader",
  "cellHeight",
  "enablePagination",
  "sortBy",
  "footer",
]);

/** Rebuild the full editor state from a cell. ADD passes `defaultCell(view)`; EDIT passes the saved
 *  cell — identical code path. */
export function cellToEditorState(cell: Cell): EditorState {
  // Keep the view RAW so serialize round-trips byte-identical (a v1 cell has none; a v2 `chart` stays
  // `chart`). The renderer canonicalizes via `cellView`; the picker via `canonicalView(state.view)`.
  const view = (cell.view as View) ?? "";
  const options = (cell.options ?? {}) as Record<string, unknown>;

  // The SQL Builder state is stowed in `cell.options.sql` by the SQL source (both raw + builder query).
  // Rehydrate it so the Builder tab reopens to the builder, not Code-only (panel-editor scope, Risks).
  const sql = (options.sql as SqlSourceState | undefined) ?? undefined;

  // The per-view typed options (legend/tooltip) — everything the editor's tabs own.
  const ownedOptions: Record<string, unknown> = {};
  const extraOptions: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(options)) {
    if (k === "sql") continue; // sql is rehydrated separately into `state.sql`
    if (OWNED_OPTION_KEYS.has(k)) ownedOptions[k] = v;
    else extraOptions[k] = v;
  }

  return {
    view,
    title: cell.title ?? "",
    description: cell.description ?? "",
    targets: cellSources(cell),
    sql,
    options: ownedOptions,
    fieldConfig: cell.fieldConfig,
    transformations: cell.transformations ?? [],
    queryOptions: cell.queryOptions,
    carry: {
      v: cell.v,
      widget_type: cell.widget_type,
      binding: cell.binding,
      action: cell.action,
      pluginVersion: cell.pluginVersion,
      targetRepr:
        cell.sources && cell.sources.length > 0
          ? "sources"
          : cell.source?.tool
            ? "source"
            : "none",
      extraOptions,
    },
  };
}

/** Serialize the editor state back to a cell, preserving `base`'s key + geometry (the edit invariant)
 *  and emitting the additive v3 shape. Absent/empty groups stay absent so a v1/v2 cell round-trips
 *  unchanged (no spurious empty `fieldConfig`/`sources` on a cell that had none). */
export function editorStateToCell(state: EditorState, base: Cell): Cell {
  // Reassemble options: the typed per-view groups + the SQL state + the untouched extras. Order is
  // irrelevant to equality (object key order isn't compared by our round-trip's deep-equal).
  const options: Record<string, unknown> = { ...state.carry.extraOptions, ...state.options };
  if (state.sql) options.sql = state.sql;

  const cell: Cell = {
    i: base.i,
    x: base.x,
    y: base.y,
    w: base.w,
    h: base.h,
    widget_type: state.carry.widget_type,
    binding: state.carry.binding,
  };

  // --- restore the version + v2/v3 groups exactly as the source cell had them (absent stays absent) ---
  if (state.carry.v !== undefined) cell.v = state.carry.v;
  if (state.view) cell.view = state.view;
  if (state.title) cell.title = state.title;
  if (state.description) cell.description = state.description;
  if (Object.keys(options).length > 0) cell.options = options;

  // Serialize targets back through the SAME shape the cell used (so a v2 cell stays single-`source`,
  // byte-identical, while a v3 cell keeps `sources[]`). A target EDIT in the Query tab flows through
  // `state.targets`, so the new query is written back here regardless of representation.
  // A cell that started with NO target (a fresh ADD) gets the v3 `sources[]` shape once the author picks
  // a real query — otherwise the authored target is silently dropped and the preview/save see no source
  // ("no data yet"). An empty/no-tool target stays absent so a truly target-less cell round-trips clean.
  const repr =
    state.carry.targetRepr === "none" && state.targets.some((t) => t.tool)
      ? "sources"
      : state.carry.targetRepr;
  if (repr === "sources") {
    cell.sources = state.targets;
  } else if (repr === "source") {
    const t = state.targets[0];
    // Reproduce the v2 `{ tool, args }` shape exactly (drop the synthetic refId/datasource cellSources
    // adds), omitting `args` when it was absent so the round-trip is byte-identical.
    if (t) cell.source = t.args === undefined ? { tool: t.tool } : { tool: t.tool, args: t.args };
  }
  if (state.carry.action) cell.action = state.carry.action;

  if (state.fieldConfig) cell.fieldConfig = state.fieldConfig;
  if (state.transformations.length > 0) cell.transformations = state.transformations;
  // Query options round-trip only when present + non-empty (absent stays absent — byte-clean).
  if (state.queryOptions && Object.values(state.queryOptions).some((v) => v !== undefined && v !== "")) {
    cell.queryOptions = state.queryOptions;
  }
  if (state.carry.pluginVersion) cell.pluginVersion = state.carry.pluginVersion;

  return cell;
}
