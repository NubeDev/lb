// Turn a variable definition into its option list (widget-config-vars Slice 2). Pure helpers — the
// async query resolution lives in `useVariableOptions`. A `custom`/`interval` variable's options are
// static; a `query`/`source` variable's options come from a tool's rows (resolved over the bridge).
//
// One model, no per-type code path beyond shaping: `query`/`source` both produce `{tool,args}` rows;
// `custom`/`interval` carry their list; `text`/`const` are single-value (no dropdown list).

import type { Variable, VariableOption } from "@/lib/vars";
import { parseCustomOptions, applyRegex, sortOptions } from "@/lib/vars";

/** A bar dropdown option — a value plus its display text. */
export interface VarOption {
  value: string;
  label: string;
}

/** The static option list for a non-query variable (`[]` for query/source/text/const). Honors the
 *  advanced `options` list (text≠value) and the `label : value` custom syntax; falls back to bare strings. */
export function staticOptions(v: Variable): VarOption[] {
  if (v.options?.length) return toBarOptions(v.options);
  if (v.type === "custom") return toBarOptions(parseCustomOptions(v.custom));
  if (v.type === "interval") return (v.interval ?? []).map((s) => ({ value: s, label: s }));
  return [];
}

/** True if a variable's options must be resolved over the bridge (a query/source/datasource tool). */
export function isQueryVariable(v: Variable): boolean {
  return (v.type === "query" || v.type === "source" || v.type === "datasource") && !!v.query?.tool;
}

/** Map the lib's `{text,value}` options to the bar's `{value,label}` shape. */
export function toBarOptions(options: VariableOption[]): VarOption[] {
  return options.map((o) => ({ value: o.value, label: o.text }));
}

/** The advanced option pipeline — regex filter/capture, then sort — applied to a resolved `{text,value}`
 *  list (advanced-variables scope). Pure; the interpolator/allValue is applied later at selection time. */
export function processOptions(v: Variable, options: VariableOption[]): VarOption[] {
  const filtered = applyRegex(options, v.regex, v.regexApplyTo);
  const sorted = sortOptions(filtered, v.sort);
  return toBarOptions(sorted);
}

/** Shape a tool result into `{text,value}` options. Accepts the shapes our read tools return:
 *  `{ rows: [...] }` (store.query / series.find), a bare array, or `{ columns, rows }`. A row yields a
 *  value (a scalar as-is, else a `value`/`name`/`label` field / the first column) and a text (a `text`/
 *  `label`/`name` field, else the value). Grafana's `__text`/`__value` column convention is honored.
 *  Deduped by value, empties dropped. */
export function rowsToOptions(result: unknown): VariableOption[] {
  const rows = extractRows(result);
  const seen = new Set<string>();
  const out: VariableOption[] = [];
  for (const row of rows) {
    const opt = rowToOption(row);
    if (opt === null || opt.value === "" || seen.has(opt.value)) continue;
    seen.add(opt.value);
    out.push(opt);
  }
  return out;
}

function extractRows(result: unknown): unknown[] {
  if (Array.isArray(result)) return result;
  if (result && typeof result === "object") {
    const obj = result as Record<string, unknown>;
    // The read tools wrap rows under a named key: `rows` (store.query), `series` (series.find/.list),
    // `samples` (series.read), `datasources` (datasource.list). Try the known keys, then fall back to
    // the first array-valued property so a new read tool's list resolves without a per-tool branch.
    for (const key of ["rows", "series", "samples", "datasources", "items", "results"]) {
      const v = obj[key];
      if (Array.isArray(v)) return v;
    }
    const firstArray = Object.values(obj).find((v) => Array.isArray(v));
    if (Array.isArray(firstArray)) return firstArray;
  }
  return [];
}

/** Read a scalar field (string/number/bool) by key, or `undefined`. */
function scalarField(obj: Record<string, unknown>, keys: string[]): string | undefined {
  for (const key of keys) {
    const v = obj[key];
    if (typeof v === "string") return v;
    if (typeof v === "number" || typeof v === "boolean") return String(v);
  }
  return undefined;
}

function rowToOption(row: unknown): VariableOption | null {
  if (typeof row === "string") return { text: row, value: row };
  if (typeof row === "number" || typeof row === "boolean") return { text: String(row), value: String(row) };
  if (row && typeof row === "object") {
    const obj = row as Record<string, unknown>;
    // Grafana's `__value`/`__text` convention, then the common value/name/label/id columns.
    let value = scalarField(obj, ["__value", "value", "name", "label", "id"]);
    const scalars = Object.values(obj).filter(
      (v) => typeof v === "string" || typeof v === "number" || typeof v === "boolean",
    );
    if (value === undefined) {
      // Fall back to the first scalar column (store.query rows may be arbitrary).
      value = scalars.length ? String(scalars[0]) : undefined;
    }
    if (value === undefined) return null;
    // A distinct display text comes only from an explicit `__text`/`text`/`label` column (Grafana's
    // `__text`/`__value` convention). The `SELECT name, code` two-column split is done via `regex`
    // named capture groups, not a positional guess — so we never surprise a plain multi-column row.
    const text = scalarField(obj, ["__text", "text", "label"]);
    return { text: text ?? value, value };
  }
  return null;
}
