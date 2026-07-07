// The display-only downsampler (builder-ergonomics session — "page freezes on big results"). The
// contract under test: identity under budget, bounded output over it, and spikes SURVIVE (the whole
// point of min/max bucketing over naive striding).

import { describe, expect, it } from "vitest";

import { downsamplePoints, downsampleRows } from "./downsample";

describe("downsamplePoints", () => {
  it("is the identity when the series fits the budget", () => {
    const points = [1, 2, 3, 4, 5];
    expect(downsamplePoints(points, 10)).toBe(points);
  });

  it("bounds the output to the budget", () => {
    const points = Array.from({ length: 50_000 }, (_, i) => Math.sin(i / 40));
    const out = downsamplePoints(points, 1500);
    expect(out.length).toBeLessThanOrEqual(1500);
    expect(out.length).toBeGreaterThan(700);
  });

  it("keeps spikes — the global min and max survive bucketing", () => {
    const points = Array.from({ length: 20_000 }, () => 0.5);
    points[7777] = 99; // one up-spike
    points[15_001] = -99; // one down-spike
    const out = downsamplePoints(points, 200);
    expect(Math.max(...out)).toBe(99);
    expect(Math.min(...out)).toBe(-99);
  });

  it("preserves encounter order inside a bucket (min-before-max vs max-before-min)", () => {
    const points = [0, 10, -10, 0, 0, 0, 0, 0];
    const out = downsamplePoints(points, 2); // one bucket → its extremes, in order
    expect(out).toEqual([10, -10]);
  });
});

describe("downsampleRows", () => {
  it("is the identity when the rows fit the budget", () => {
    const rows = [{ a: 1 }, { a: 2 }];
    expect(downsampleRows(rows, 5)).toBe(rows);
  });

  it("bounds the output and always keeps the first and last row", () => {
    const rows = Array.from({ length: 10_000 }, (_, i) => ({ i }));
    const out = downsampleRows(rows, 100);
    expect(out.length).toBe(100);
    expect(out[0]).toBe(rows[0]);
    expect(out[out.length - 1]).toBe(rows[rows.length - 1]);
  });
});
