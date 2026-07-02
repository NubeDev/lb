import { describe, expect, it } from "vitest";
import { layoutPositions, STACK_OFFSET } from "./layout";

const at = (x: number, y: number) => ({ metadata: { position: { x, y } } });
const W = 200;

describe("layoutPositions", () => {
  it("grids components when every position is (0,0)", () => {
    const pos = layoutPositions([at(0, 0), at(0, 0), at(0, 0), at(0, 0)], W);
    // 4 nodes → 2 cols; expect a 2×2 grid, no two share a cell.
    const keys = new Set(pos.map((p) => `${p.x},${p.y}`));
    expect(keys.size).toBe(4);
    expect(pos[0]).toEqual({ x: 0, y: 0 });
    expect(pos[1]).toEqual({ x: W + 60, y: 0 });
  });

  it("keeps distinct saved positions untouched", () => {
    const pos = layoutPositions([at(10, 20), at(300, 400)], W);
    expect(pos).toEqual([
      { x: 10, y: 20 },
      { x: 300, y: 400 },
    ]);
  });

  it("de-stacks exact duplicates diagonally, first one unmoved", () => {
    const pos = layoutPositions([at(100, 100), at(100, 100), at(100, 100)], W);
    expect(pos).toEqual([
      { x: 100, y: 100 },
      { x: 100 + STACK_OFFSET, y: 100 + STACK_OFFSET },
      { x: 100 + 2 * STACK_OFFSET, y: 100 + 2 * STACK_OFFSET },
    ]);
  });

  it("only nudges the colliding position, not other nodes", () => {
    const pos = layoutPositions([at(100, 100), at(500, 500), at(100, 100)], W);
    expect(pos[1]).toEqual({ x: 500, y: 500 });
    expect(pos[2]).toEqual({ x: 100 + STACK_OFFSET, y: 100 + STACK_OFFSET });
  });
});
