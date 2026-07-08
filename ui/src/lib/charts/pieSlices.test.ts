// capPieSlices — the pie's readable-slice cap (regression: a timeseries fed to a pie drew one sliver
// per timestamp behind a legend wall; slices must merge duplicates, keep the top by value, and bucket
// the tail into an explicit "Other (n)").

import { describe, expect, it } from "vitest";

import { capPieSlices, MAX_PIE_SLICES } from "./pieSlices";

const slices = (n: number) => Array.from({ length: n }, (_, i) => ({ name: `c${i}`, value: i + 1 }));

describe("capPieSlices", () => {
  it("passes a small slice list through unchanged", () => {
    expect(capPieSlices(slices(5))).toHaveLength(5);
  });

  it("merges duplicate names by summing their values", () => {
    const out = capPieSlices([
      { name: "a", value: 1 },
      { name: "a", value: 2 },
      { name: "b", value: 5 },
    ]);
    expect(out).toEqual([
      { name: "a", value: 3 },
      { name: "b", value: 5 },
    ]);
  });

  it("caps a high-cardinality list to the top slices + one Other bucket", () => {
    const out = capPieSlices(slices(100));
    expect(out).toHaveLength(MAX_PIE_SLICES);
    const other = out[out.length - 1]!;
    expect(other.name).toBe(`Other (${100 - (MAX_PIE_SLICES - 1)})`);
    // The kept slices are the largest; Other carries the whole tail's sum (nothing dropped).
    const total = slices(100).reduce((a, s) => a + s.value, 0);
    expect(out.reduce((a, s) => a + s.value, 0)).toBe(total);
    expect(out[0]!.value).toBe(100);
  });
});
