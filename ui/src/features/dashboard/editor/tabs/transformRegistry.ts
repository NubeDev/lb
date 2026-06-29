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

/** One catalog entry — the id + a human label for the "Add transformation" dropdown. */
export interface TransformDef {
  id: TransformId;
  label: string;
}

/** The catalog, in dropdown order. */
export const TRANSFORM_DEFS: TransformDef[] = [
  { id: "reduce", label: "Reduce" },
  { id: "organize", label: "Organize fields" },
  { id: "filterFieldsByName", label: "Filter fields by name" },
  { id: "filterByValue", label: "Filter data by values" },
  { id: "groupBy", label: "Group by" },
  { id: "joinByField", label: "Join by field" },
  { id: "calculateField", label: "Add field from calculation" },
  { id: "sortBy", label: "Sort by" },
  { id: "limit", label: "Limit" },
  { id: "merge", label: "Merge series/tables" },
  { id: "seriesToRows", label: "Series to rows" },
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
