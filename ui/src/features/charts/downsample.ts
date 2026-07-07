// Display-only downsampling (data-studio builder-ergonomics session). An SVG chart cannot honestly
// show more points than it has pixels — past the budget every extra row is pure DOM cost (the
// "page freezes on big results" bug: preview + 6 gallery thumbnails × N rows of SVG nodes). This
// bounds what is DRAWN, never what is fetched, transformed, or saved (rule 9 untouched: the points
// are the real rows, bucketed).
//
// One responsibility: shrink a series to a point budget without lying about its shape.

/** Min/max-per-bucket downsample: each bucket contributes its extremes (in encounter order), so
 *  spikes survive — the failure mode of naive striding. ≤ budget points out; identity when the
 *  series already fits. Budget floor 2 (first/last always meaningful). */
export function downsamplePoints(points: number[], budget: number): number[] {
  const cap = Math.max(2, Math.floor(budget));
  if (points.length <= cap) return points;
  const buckets = Math.max(1, Math.floor(cap / 2));
  const out: number[] = [];
  const size = points.length / buckets;
  for (let b = 0; b < buckets; b++) {
    const start = Math.floor(b * size);
    const end = Math.min(points.length, Math.max(start + 1, Math.floor((b + 1) * size)));
    let minI = start;
    let maxI = start;
    for (let i = start + 1; i < end; i++) {
      if (points[i] < points[minI]) minI = i;
      if (points[i] > points[maxI]) maxI = i;
    }
    if (minI === maxI) out.push(points[minI]);
    else if (minI < maxI) out.push(points[minI], points[maxI]);
    else out.push(points[maxI], points[minI]);
  }
  return out;
}

/** Row-object downsample for the multi-series plot path: one representative row per bucket, first
 *  and last always kept (a per-series min/max would tear the shared-x rows apart — the honest
 *  per-series treatment is the single-series `downsamplePoints`). */
export function downsampleRows<T>(rows: T[], budget: number): T[] {
  const cap = Math.max(2, Math.floor(budget));
  if (rows.length <= cap) return rows;
  const out: T[] = [];
  const step = (rows.length - 1) / (cap - 1);
  for (let i = 0; i < cap; i++) out.push(rows[Math.round(i * step)]);
  return out;
}
