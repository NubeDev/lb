// Panel-rows membership + collapse geometry (panel-rows scope, "Unit"). `rowMembers` is positional —
// the cells between a row's `y` and the next row's `y` — and independent of collapse; `visibleCells`
// drops a collapsed row's members (keeping their record geometry). No gateway needed: pure geometry.

import { describe, expect, it } from "vitest";

import type { Cell } from "./dashboard.types";
import {
  isRow,
  isCollapsed,
  rows,
  rowMembers,
  ungroupedCells,
  visibleCells,
  ROW_W,
  ROW_H,
} from "./rows";

/** A minimal cell — geometry + optional view/options; the fields `rows.ts` reads. */
function cell(i: string, y: number, view?: string, opts?: Record<string, unknown>): Cell {
  return {
    i,
    x: 0,
    y,
    w: view === "row" ? ROW_W : 6,
    h: view === "row" ? ROW_H : 4,
    widget_type: "chart",
    binding: { series: "s" },
    ...(view ? { view: view as Cell["view"] } : {}),
    ...(opts ? { options: opts } : {}),
  };
}

describe("panel rows — positional membership", () => {
  it("rowMembers returns exactly the cells between a row's y and the next row's y", () => {
    const cells = [
      cell("r1", 0, "row"),
      cell("a", 1),
      cell("b", 5),
      cell("r2", 9, "row"),
      cell("c", 10),
    ];
    expect(rowMembers(cells, cells[0]).map((c) => c.i)).toEqual(["a", "b"]);
    expect(rowMembers(cells, cells[3]).map((c) => c.i)).toEqual(["c"]);
  });

  it("a dashboard with no rows returns every cell as ungrouped, none as members", () => {
    const cells = [cell("a", 0), cell("b", 4), cell("c", 8)];
    expect(rows(cells)).toEqual([]);
    expect(ungroupedCells(cells).map((c) => c.i)).toEqual(["a", "b", "c"]);
  });

  it("two adjacent rows partition their members cleanly", () => {
    const cells = [
      cell("r1", 0, "row"),
      cell("a", 1),
      cell("r2", 2, "row"),
      cell("b", 3),
    ];
    expect(rowMembers(cells, cells[0]).map((c) => c.i)).toEqual(["a"]);
    expect(rowMembers(cells, cells[2]).map((c) => c.i)).toEqual(["b"]);
  });

  it("a trailing row owns everything below it", () => {
    const cells = [cell("a", 0), cell("r1", 5, "row"), cell("b", 6), cell("c", 9)];
    expect(ungroupedCells(cells).map((c) => c.i)).toEqual(["a"]);
    expect(rowMembers(cells, cells[1]).map((c) => c.i)).toEqual(["b", "c"]);
  });

  it("membership is positional — a collapsed row's members are still returned", () => {
    const cells = [cell("r1", 0, "row", { collapsed: true }), cell("a", 1), cell("b", 2)];
    expect(isCollapsed(cells[0])).toBe(true);
    expect(rowMembers(cells, cells[0]).map((c) => c.i)).toEqual(["a", "b"]);
  });

  it("rowMembers on a non-row cell is an empty no-op", () => {
    const cells = [cell("a", 0), cell("r1", 1, "row")];
    expect(isRow(cells[0])).toBe(false);
    expect(rowMembers(cells, cells[0])).toEqual([]);
  });
});

describe("panel rows — collapse render transform", () => {
  it("visibleCells drops a collapsed row's members but keeps the header + other cells", () => {
    const cells = [
      cell("top", 0),
      cell("r1", 1, "row", { collapsed: true }),
      cell("a", 2),
      cell("b", 3),
      cell("r2", 9, "row"),
      cell("c", 10),
    ];
    expect(visibleCells(cells).map((c) => c.i)).toEqual(["top", "r1", "r2", "c"]);
  });

  it("an expanded row hides nothing — visibleCells is identity", () => {
    const cells = [cell("r1", 0, "row"), cell("a", 1), cell("b", 2)];
    expect(visibleCells(cells)).toBe(cells);
  });

  it("collapse never mutates the members' stored geometry (they keep their real y)", () => {
    const cells = [cell("r1", 0, "row", { collapsed: true }), cell("a", 3)];
    // The member is absent from the render list...
    expect(visibleCells(cells).some((c) => c.i === "a")).toBe(false);
    // ...but its record geometry is untouched (expand restores it at y:3).
    expect(cells.find((c) => c.i === "a")?.y).toBe(3);
  });
});
