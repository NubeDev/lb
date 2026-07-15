// Pure panel-rows math: positional membership, the ungrouped top region, collapse visibility.

import { describe, expect, it } from "vitest";

import type { Cell } from "./dashboard.types";
import { isRow, rowMembers, rows, ungroupedCells, visibleCells } from "./rows";

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

describe("rows model", () => {
  const board: Cell[] = [
    cell("top", 0), // above the first row → ungrouped
    row("r1", 4),
    cell("a", 5),
    cell("b", 6),
    row("r2", 10, { collapsed: true }),
    cell("c", 11),
  ];

  it("identifies row cells and orders section boundaries by y", () => {
    expect(isRow(board[1])).toBe(true);
    expect(isRow(board[0])).toBe(false);
    expect(rows(board).map((r) => r.i)).toEqual(["r1", "r2"]);
  });

  it("derives membership positionally: y in [row.y, nextRow.y)", () => {
    expect(rowMembers(board, board[1]).map((c) => c.i)).toEqual(["a", "b"]);
    // A trailing row owns everything below it.
    expect(rowMembers(board, board[4]).map((c) => c.i)).toEqual(["c"]);
    // A non-row cell is a defensive no-op.
    expect(rowMembers(board, board[0])).toEqual([]);
  });

  it("puts cells above the first row in the ungrouped region", () => {
    expect(ungroupedCells(board).map((c) => c.i)).toEqual(["top"]);
  });

  it("hides a collapsed row's members but keeps the header (render-time only)", () => {
    const shown = visibleCells(board).map((c) => c.i);
    expect(shown).toContain("r2");
    expect(shown).not.toContain("c");
    // Never mutates the stored geometry.
    expect(board.find((c) => c.i === "c")?.y).toBe(11);
  });

  it("passes everything through when no row is collapsed", () => {
    const open = board.map((c) => (c.i === "r2" ? { ...c, options: {} } : c));
    expect(visibleCells(open)).toHaveLength(open.length);
  });
});
