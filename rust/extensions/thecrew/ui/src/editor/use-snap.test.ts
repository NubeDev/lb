// use-snap.ts unit tests: grid rounding, anchor magnetism beats grid, radius edge.

import { describe, expect, it } from "vitest";
import { snap, snapToGrid } from "./use-snap";

describe("snapToGrid", () => {
  it("rounds to the nearest step", () => {
    expect(snapToGrid(11, 8)).toBe(8);
    expect(snapToGrid(12, 8)).toBe(16); // .5 rounds up
    expect(snapToGrid(-11, 8)).toBe(-8);
  });
});

describe("snap", () => {
  const anchors = [
    { shapeId: "fan-1", name: "in", x: 100, y: 0 },
    { shapeId: "fan-1", name: "out", x: 164, y: 0 },
  ];

  it("passes through with no grid and no anchors", () => {
    expect(snap(3.7, -2.2)).toEqual({ x: 3.7, y: -2.2 });
  });

  it("grid-snaps when grid is set", () => {
    expect(snap(13, 21, { grid: 8 })).toEqual({ x: 16, y: 24 });
  });

  it("anchor magnetism wins over grid and returns the connect affordance", () => {
    const r = snap(103, 4, { grid: 8, anchors, anchorRadius: 16 });
    expect(r).toEqual({ x: 100, y: 0, anchor: { shapeId: "fan-1", name: "in" } });
  });

  it("picks the NEAREST anchor within radius", () => {
    const r = snap(140, 0, { anchors, anchorRadius: 100 });
    expect(r.anchor?.name).toBe("out");
  });

  it("ignores anchors outside the radius (falls back to grid)", () => {
    const r = snap(130, 0, { grid: 8, anchors, anchorRadius: 16 });
    expect(r).toEqual({ x: 128, y: 0 });
  });
});
