// capPieSlices — make a pie honest at any cardinality. A pie is readable up to ~a dozen slices; a
// high-cardinality category (each timestamp of a timeseries, 600 point ids) turns it into a legend wall
// with invisible slivers. This helper (shared by BOTH pie renderers — the piechart panel and the plot
// preview) first MERGES duplicate names (sum — same category, one slice), then keeps the top slices by
// value and buckets the tail into one explicit "Other (n)" slice. Nothing is hidden — the tail is
// aggregated, visibly, never dropped. One responsibility: names+values → a bounded slice list.

export interface PieSlice {
  name: string;
  value: number;
}

/** How many named slices a pie keeps before the tail collapses into "Other (n)". */
export const MAX_PIE_SLICES = 12;

export function capPieSlices<T extends PieSlice>(
  slices: T[],
  max: number = MAX_PIE_SLICES,
): Array<PieSlice & { otherCount?: number }> {
  // Merge duplicate names first (sum) — the same category is one slice, whatever the row count.
  const byName = new Map<string, number>();
  for (const s of slices) byName.set(s.name, (byName.get(s.name) ?? 0) + s.value);
  const merged = [...byName.entries()].map(([name, value]) => ({ name, value }));
  if (merged.length <= max) return merged;

  const sorted = merged.slice().sort((a, b) => b.value - a.value);
  const kept = sorted.slice(0, max - 1);
  const tail = sorted.slice(max - 1);
  const other = tail.reduce((a, s) => a + s.value, 0);
  return [...kept, { name: `Other (${tail.length})`, value: other, otherCount: tail.length }];
}
