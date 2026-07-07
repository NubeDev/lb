// Coerce an arbitrary source value to a number for the numeric v2 views (chart/stat/gauge). A source
// result is any JSON value; a non-numeric one yields `null` so a view can show the raw value (stat) or
// skip the point (chart) — never a fabricated 0 (the no-mock rule). Shared by the v2 view renderers.

export function asNumber(value: unknown): number | null {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string") {
    const n = Number(value);
    return Number.isFinite(n) ? n : null;
  }
  if (typeof value === "boolean") return value ? 1 : 0;
  return null;
}

/** Pull the numeric value of a row (the chart/stat point), or null if non-numeric.
 *  Order: the canonical `value`/`payload` column (the series/ingest shape), else the FIRST column that
 *  is an actual number — the deterministic contract for arbitrary SQL frames (`federation.query`/
 *  `store.query` rows have no `value` column; a summary row like `{point_id, reading_count, avg_value}`
 *  charts its first numeric column, and the user picks a different one by shaping the SELECT). The
 *  scan accepts only real numbers — never numeric-looking strings (an id/timestamp string must not
 *  become a fabricated point). */
export function rowNumber(row: Record<string, unknown>): number | null {
  const direct = asNumber(row.value ?? row.payload);
  if (direct !== null) return direct;
  // The column scan applies ONLY to rows with no value/payload key at all (a SQL frame). A series row
  // whose payload is null/non-numeric stays null — never silently charts `seq`/`ts` instead.
  if (row && typeof row === "object" && !("value" in row) && !("payload" in row)) {
    for (const v of Object.values(row)) {
      if (typeof v === "number" && Number.isFinite(v)) return v;
    }
  }
  return asNumber(row);
}
