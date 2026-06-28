// Turn a variable definition into its option list (widget-config-vars Slice 2). Pure helpers — the
// async query resolution lives in `useVariableOptions`. A `custom`/`interval` variable's options are
// static; a `query`/`source` variable's options come from a tool's rows (resolved over the bridge).
//
// One model, no per-type code path beyond shaping: `query`/`source` both produce `{tool,args}` rows;
// `custom`/`interval` carry their list; `text`/`const` are single-value (no dropdown list).

import type { Variable } from "@/lib/vars";

/** A bar dropdown option — a value plus its display text. */
export interface VarOption {
  value: string;
  label: string;
}

/** The static option list for a non-query variable (`[]` for query/source/text/const). */
export function staticOptions(v: Variable): VarOption[] {
  if (v.type === "custom") return (v.custom ?? []).map((s) => ({ value: s, label: s }));
  if (v.type === "interval") return (v.interval ?? []).map((s) => ({ value: s, label: s }));
  return [];
}

/** True if a variable's options must be resolved over the bridge (a query/source tool). */
export function isQueryVariable(v: Variable): boolean {
  return (v.type === "query" || v.type === "source") && !!v.query?.tool;
}

/** Shape a tool result into options. Accepts the shapes our read tools return: `{ rows: [...] }`
 *  (store.query / series.find), a bare array, or `{ columns, rows }`. A row is reduced to a string —
 *  a scalar as-is, else the first column / a `value`/`name`/`label` field. Deduped, empties dropped. */
export function rowsToOptions(result: unknown): VarOption[] {
  const rows = extractRows(result);
  const seen = new Set<string>();
  const out: VarOption[] = [];
  for (const row of rows) {
    const value = rowToValue(row);
    if (value === null || value === "" || seen.has(value)) continue;
    seen.add(value);
    out.push({ value, label: value });
  }
  return out;
}

function extractRows(result: unknown): unknown[] {
  if (Array.isArray(result)) return result;
  if (result && typeof result === "object") {
    const r = (result as { rows?: unknown }).rows;
    if (Array.isArray(r)) return r;
  }
  return [];
}

function rowToValue(row: unknown): string | null {
  if (typeof row === "string") return row;
  if (typeof row === "number" || typeof row === "boolean") return String(row);
  if (row && typeof row === "object") {
    const obj = row as Record<string, unknown>;
    for (const key of ["value", "name", "label", "id"]) {
      const v = obj[key];
      if (typeof v === "string") return v;
      if (typeof v === "number" || typeof v === "boolean") return String(v);
    }
    // Fall back to the first scalar column (store.query rows may be arbitrary).
    const first = Object.values(obj).find(
      (v) => typeof v === "string" || typeof v === "number" || typeof v === "boolean",
    );
    return first === undefined ? null : String(first);
  }
  return null;
}
