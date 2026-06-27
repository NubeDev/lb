// Coerce a sample payload to a number for the numeric widgets (chart/gauge/stat). A sample payload
// is any SurrealDB-typed value (ingest scope); a non-numeric one yields `null` so the widget can show
// the raw value (stat) or skip the point (chart) — never a fabricated 0 (the no-mock rule).

export function asNumber(payload: unknown): number | null {
  if (typeof payload === "number" && Number.isFinite(payload)) return payload;
  if (typeof payload === "string") {
    const n = Number(payload);
    return Number.isFinite(n) ? n : null;
  }
  return null;
}
