// The drag/resize merge: geometry-only updates onto the record, and the moved-row member-carry
// Δy math (hidden/collapsed members shift; layout-touched members are authoritative).

import { describe, expect, it } from "vitest";

import type { Cell } from "./dashboard.types";
import { mergeLayout } from "./layout";

const cell = (i: string, y: number, extra: Partial<Cell> = {}): Cell => ({
  i,
  x: 0,
  y,
  w: 6,
  h: 4,
  widget_type: "chart",
  binding: { series: "s" },
  ...extra,
});

const row = (i: string, y: number, options?: Record<string, unknown>): Cell =>
  cell(i, y, { v: 2, view: "row", w: 12, h: 1, options });

describe("mergeLayout", () => {
  it("takes geometry from the layout and keeps everything else from the cell", () => {
    const cells = [cell("a", 0, { title: "keep me", options: { unit: "°C" } })];
    const out = mergeLayout(cells, [{ i: "a", x: 3, y: 2, w: 4, h: 5 }]);
    expect(out[0]).toMatchObject({ i: "a", x: 3, y: 2, w: 4, h: 5, title: "keep me" });
    expect(out[0].options).toEqual({ unit: "°C" });
  });

  it("passes cells absent from the layout through unchanged", () => {
    const cells = [cell("a", 0), cell("b", 4)];
    const out = mergeLayout(cells, [{ i: "a", x: 0, y: 1, w: 6, h: 4 }]);
    expect(out.find((c) => c.i === "b")).toEqual(cells[1]);
  });

  it("carries a collapsed row's hidden members by the row's Δy", () => {
    // r collapsed: member m is NOT in the layout (hidden), but the row moved down by 3.
    const cells = [row("r", 2, { collapsed: true }), cell("m", 3)];
    const out = mergeLayout(cells, [{ i: "r", x: 0, y: 5, w: 12, h: 1 }]);
    expect(out.find((c) => c.i === "m")?.y).toBe(6);
  });

  it("lets the layout win for members it repositioned itself", () => {
    // r expanded: m is on-screen, react-grid-layout already placed it — the layout is authoritative.
    const cells = [row("r", 2), cell("m", 3)];
    const out = mergeLayout(cells, [
      { i: "r", x: 0, y: 5, w: 12, h: 1 },
      { i: "m", x: 0, y: 6, w: 6, h: 4 },
    ]);
    expect(out.find((c) => c.i === "m")?.y).toBe(6);
  });

  it("does not shift members when the row did not move", () => {
    const cells = [row("r", 2, { collapsed: true }), cell("m", 3)];
    const out = mergeLayout(cells, [{ i: "r", x: 0, y: 2, w: 12, h: 1 }]);
    expect(out.find((c) => c.i === "m")?.y).toBe(3);
  });
});
