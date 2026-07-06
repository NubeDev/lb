// The ONE compact duration formatter used wherever a query/run/elapsed wall-clock
// is shown to the user (query-workbench run bar, panel-builder QueryStatusBar, …). Pure, locale-
// independent, no DOM — one verb per file (FILE-LAYOUT). Mirrors the `<1 ms / N ms / N.NN s`
// convention the panel-builder already shipped; extracted so the two surfaces never drift.

/** Format a duration in milliseconds compactly for inline display.
 *  - undefined / null / NaN → null (caller renders nothing — the field was absent, not "0").
 *  - <1 ms → "<1 ms" (sub-millisecond reads as instant; a "0 ms" reads as a bug).
 *  - <1 s  → "N ms" (rounded).
 *  - ≥1 s  → "N.NN s" (two decimals — enough resolution to spot a 1.05→1.20 s regression). */
export function formatMs(n: number | undefined | null): string | null {
  if (n === undefined || n === null || !Number.isFinite(n)) return null;
  if (n < 1) return "<1 ms";
  if (n < 1000) return `${Math.round(n)} ms`;
  return `${(n / 1000).toFixed(2)} s`;
}
