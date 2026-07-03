// The TS transform REGISTRY (viz transformations scope) — the supported transformer ids + a sane
// default-options factory, and NOTHING ELSE. There is NO executor here (invariant B, README phasing):
// the backend (`lb-viz` / `viz.query`) RUNS the pipeline; this module only describes what the Transform
// tab can ADD and what default `options` bag each id is born with, so add == edit and a fresh transform
// is immediately valid. Ids are Grafana's verbatim (so an imported dashboard's transforms map 1:1).
// One responsibility: the id catalog + defaults.

/** A supported transformer id (Grafana parity). */
export type TransformId =
  | "reduce"
  | "organize"
  | "filterFieldsByName"
  | "filterByValue"
  | "groupBy"
  | "joinByField"
  | "calculateField"
  | "sortBy"
  | "limit"
  | "merge"
  | "seriesToRows";

/** One catalog entry — the id + a human label + a one-line description for the searchable add picker. */
export interface TransformDef {
  id: TransformId;
  label: string;
  description: string;
}

/** The catalog, in dropdown order. Descriptions mirror Grafana's one-liners so the searchable picker
 *  reads like the real thing. */
export const TRANSFORM_DEFS: TransformDef[] = [
  { id: "reduce", label: "Reduce", description: "Reduce all rows to a single value per field" },
  { id: "organize", label: "Organize fields", description: "Reorder, hide, and rename fields" },
  { id: "filterFieldsByName", label: "Filter fields by name", description: "Keep only fields matching a name or regex" },
  { id: "filterByValue", label: "Filter data by values", description: "Keep rows whose field values match conditions" },
  { id: "groupBy", label: "Group by", description: "Group rows and aggregate each group" },
  { id: "joinByField", label: "Join by field", description: "Join frames on a shared field" },
  { id: "calculateField", label: "Add field from calculation", description: "Derive a new field from a formula" },
  { id: "sortBy", label: "Sort by", description: "Sort rows by a field" },
  { id: "limit", label: "Limit", description: "Keep the first N rows" },
  { id: "merge", label: "Merge series/tables", description: "Merge multiple frames into one" },
  { id: "seriesToRows", label: "Series to rows", description: "Turn multiple series into labelled rows" },
];

/** A human label for an id (falls back to the id for an unknown/imported one). */
export function transformLabel(id: string): string {
  return TRANSFORM_DEFS.find((d) => d.id === id)?.label ?? id;
}

/** The sane default `options` bag a freshly-added transform of `id` is born with (Grafana defaults).
 *  The backend reads these; the tab edits them. NOT an executor — just the starting config. */
export function defaultOptions(id: TransformId): Record<string, unknown> {
  switch (id) {
    case "reduce":
      return { reducers: ["lastNotNull"], mode: "seriesToRows" };
    case "organize":
      return { excludeByName: {}, indexByName: {}, renameByName: {} };
    case "filterFieldsByName":
      return { include: { names: [] } };
    case "filterByValue":
      return { type: "include", match: "all", filters: [] };
    case "groupBy":
      return { fields: {} };
    case "joinByField":
      return { byField: "", mode: "outer" };
    case "calculateField":
      return { mode: "binary", alias: "", binary: { left: "", operator: "+", right: "" }, replaceFields: false };
    case "sortBy":
      return { fields: {}, sort: [{ field: "", desc: false }] };
    case "limit":
      return { limitField: 10 };
    case "merge":
      return {};
    case "seriesToRows":
      return {};
    default:
      return {};
  }
}
