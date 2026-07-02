// Field typing for the plot builder — inspect a set of query rows and label each column `time`,
// `number`, or `category`. The builder shows these kinds as pills (so a user knows what's safe on the
// x vs the y axis) and the auto-suggester (`suggestPlot`) uses them to pick a default chart. Inference
// is by SAMPLED VALUES, not column names, so it works for any datasource frame (SurrealDB, series,
// federation) without a schema.
//
// One responsibility: rows → typed fields. No spec, no rendering.

export type FieldKind = "time" | "number" | "category";

export interface FieldInfo {
  name: string;
  kind: FieldKind;
}

const ISO_DATE = /^\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}/;
/** A plausible epoch window: 2001-09-09 (1e9 s) … year ~33658 (1e12 s) as seconds, or the same in ms. */
const EPOCH_S_MIN = 1_000_000_000;
const EPOCH_MS_MAX = 4_102_444_800_000; // 2100-01-01 in ms

/** Classify one already-narrowed sample value, or `null` when it carries no type signal (null/empty). */
function classify(value: unknown): FieldKind | null {
  if (value == null || value === "") return null;
  if (value instanceof Date) return "time";
  if (typeof value === "number") {
    // A large integer in the epoch band reads as a timestamp; other numbers are quantities.
    if (Number.isInteger(value) && value >= EPOCH_S_MIN && value <= EPOCH_MS_MAX) return "time";
    return "number";
  }
  if (typeof value === "string") {
    if (ISO_DATE.test(value)) return "time";
    // A numeric string is a number (SurrealDB/CSV frames often stringify numbers).
    if (value.trim() !== "" && Number.isFinite(Number(value))) return "number";
    return "category";
  }
  if (typeof value === "boolean") return "category";
  return "category";
}

/** The dominant kind across a column's sampled values. Ties and mixed columns fall back to `category`
 *  (safe on any axis); a column that's all-null is treated as a category so it still appears in the
 *  picker rather than vanishing. */
function columnKind(values: unknown[]): FieldKind {
  const tally: Record<FieldKind, number> = { time: 0, number: 0, category: 0 };
  let seen = 0;
  for (const v of values) {
    const k = classify(v);
    if (k === null) continue;
    tally[k] += 1;
    seen += 1;
  }
  if (seen === 0) return "category";
  // A single non-number breaks a "number" column (mixed → category); time wins over number when it
  // dominates, matching the host picker's "temporal first column" bias.
  if (tally.time > 0 && tally.time >= tally.category) return "time";
  if (tally.number === seen) return "number";
  if (tally.number > 0 && tally.category === 0) return "number";
  return "category";
}

/** Infer the kind of every column present in the rows, preserving the first-seen column order (so the
 *  builder lists fields the way the query returned them). Samples up to `sample` rows for speed. */
export function inferFields(rows: Array<Record<string, unknown>>, sample = 200): FieldInfo[] {
  const order: string[] = [];
  const cols = new Map<string, unknown[]>();
  const n = Math.min(rows.length, sample);
  for (let i = 0; i < n; i++) {
    for (const key of Object.keys(rows[i])) {
      let bucket = cols.get(key);
      if (!bucket) {
        bucket = [];
        cols.set(key, bucket);
        order.push(key);
      }
      bucket.push(rows[i][key]);
    }
  }
  return order.map((name) => ({ name, kind: columnKind(cols.get(name) ?? []) }));
}

/** The numeric fields, in order — the candidate y series. */
export function numericFields(fields: FieldInfo[]): string[] {
  return fields.filter((f) => f.kind === "number").map((f) => f.name);
}

/** The best default x field: the first temporal column, else the first categorical, else the first
 *  column of any kind. Mirrors the host `pick_chart` "temporal first, else categorical" order. */
export function defaultXField(fields: FieldInfo[]): string | undefined {
  return (
    fields.find((f) => f.kind === "time")?.name ??
    fields.find((f) => f.kind === "category")?.name ??
    fields[0]?.name
  );
}
