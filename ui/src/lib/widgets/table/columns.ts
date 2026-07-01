// The shared table column-model (widget-kit scope, Phase 1) — the ONE place BOTH table renderers
// (`dashboard/views/table/TablePanel` read-only, `channel/ResponseTable` row-controlled) turn a frame's
// rows + its `fieldConfig` into the ordered, presentation-resolved columns they render. Before this each
// renderer introspected raw keys and hand-labelled headers (or ignored fieldConfig entirely), so a
// `maxRuns` header and a "Max Runs" form label drifted. Funnelling both through
// {@link resolveFieldPresentation} + {@link humanize} retires that. One responsibility: rows + fieldConfig
// → the columns to render. No React (FILE-LAYOUT — the renderers own the markup).

import type { FieldConfig } from "@/lib/dashboard";
import { resolveFieldPresentation } from "@/lib/widgets/presentation/resolve";

/** One resolved table column — the raw row `key`, its presentation-resolved `header`, and its
 *  `description` (help/tooltip). Hidden columns are DROPPED from the returned list, never emitted. */
export interface TableColumn {
  key: string;
  header: string;
  description?: string;
}

/** The union of keys across the rows, in first-seen order — the introspected raw columns. Shared by both
 *  renderers (was duplicated `columnsOf` in each). */
export function columnsOf(rows: Array<Record<string, unknown>>): string[] {
  const seen: string[] = [];
  for (const row of rows) for (const k of Object.keys(row)) if (!seen.includes(k)) seen.push(k);
  return seen;
}

/** Read one field's presentation hints off a `fieldConfig` — the effective FieldOptions for a `byName`
 *  match, reduced to the presentation keys the resolver reads (`displayName`==label, `description`,
 *  `hide`, `order`). We read the DEFAULTS + any `byName:<field>` override (last-wins), mirroring
 *  `resolveFieldOptions` but for presentation only (formatting stays owned by `fieldconfig/`). Kept local
 *  and dependency-light so the library doesn't import a dashboard feature. */
function presentationHintsFor(fc: FieldConfig | undefined, field: string) {
  const defaults = fc?.defaults ?? {};
  let label = defaults.displayName;
  let description = defaults.description;
  let hide = (defaults as { hide?: boolean }).hide;
  let order = (defaults as { order?: number }).order;
  for (const over of fc?.overrides ?? []) {
    if (over.matcher.id !== "byName" || over.matcher.options !== field) continue;
    for (const prop of over.properties) {
      if (prop.id === "displayName") label = prop.value as string;
      else if (prop.id === "description") description = prop.value as string;
      else if (prop.id === "hide") hide = prop.value as boolean;
      else if (prop.id === "order") order = prop.value as number;
    }
  }
  return { displayName: label, description, hide, order };
}

/** Resolve the ordered, presentation-resolved columns a table renders from its `rows` + optional
 *  `fieldConfig`. Every header funnels through {@link resolveFieldPresentation} (label override →
 *  humanize fallback); a column whose field is `hide`-marked is DROPPED; an explicit `order` reorders
 *  (ascending, stable) — a column with NO `order` keeps its first-seen position AFTER the ordered ones,
 *  so absent `order` never reorders implicitly. */
export function resolveColumns(
  rows: Array<Record<string, unknown>>,
  fc?: FieldConfig,
): TableColumn[] {
  const keys = columnsOf(rows);
  const resolved = keys.map((key, i) => {
    const p = resolveFieldPresentation(key, presentationHintsFor(fc, key));
    return { key, header: p.label, description: p.description, hidden: p.hidden, order: p.order, i };
  });
  const shown = resolved.filter((c) => !c.hidden);
  // Stable order: fields with an explicit `order` sort ascending; fields without keep first-seen order,
  // sorted AFTER any ordered field at their original index. (Absent order → natural order preserved.)
  shown.sort((a, b) => {
    if (a.order !== undefined && b.order !== undefined) return a.order - b.order || a.i - b.i;
    if (a.order !== undefined) return -1;
    if (b.order !== undefined) return 1;
    return a.i - b.i;
  });
  return shown.map(({ key, header, description }) => ({ key, header, description }));
}

/** Render one cell VALUE as honest text — an object as JSON (a nested `action` becomes readable text,
 *  not a thrown blob), null/undefined as empty, everything else verbatim. The numeric-format path
 *  (thresholds/units) stays in `TablePanel` via `fieldconfig/format`; this is the plain fallback both
 *  renderers share for non-numeric cells. */
export function cellText(v: unknown): string {
  if (v == null) return "";
  return typeof v === "object" ? JSON.stringify(v) : String(v);
}
