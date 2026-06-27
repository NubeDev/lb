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

/** Pull the numeric `value`/`payload` of a row (the chart/stat point), or null if non-numeric. */
export function rowNumber(row: Record<string, unknown>): number | null {
  return asNumber(row.value ?? row.payload ?? row);
}
