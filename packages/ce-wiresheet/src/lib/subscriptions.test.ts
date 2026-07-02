import { describe, expect, it } from "vitest";
import { diffSets } from "./subscriptions";

const sort = (a: number[]) => [...a].sort((x, y) => x - y);

describe("diffSets", () => {
  it("reports adds (in desired, not current) and removes (in current, not desired)", () => {
    const d = diffSets(new Set([1, 2, 3]), new Set([2, 3, 4]));
    expect(sort(d.added)).toEqual([4]);
    expect(sort(d.removed)).toEqual([1]);
  });

  it("is empty when the sets match", () => {
    const d = diffSets(new Set([1, 2]), new Set([2, 1]));
    expect(d.added).toEqual([]);
    expect(d.removed).toEqual([]);
  });

  it("handles empty current (initial subscribe) and empty desired (unsubscribe all)", () => {
    expect(sort(diffSets(new Set(), new Set([5, 6])).added)).toEqual([5, 6]);
    expect(sort(diffSets(new Set([5, 6]), new Set()).removed)).toEqual([5, 6]);
  });
});
