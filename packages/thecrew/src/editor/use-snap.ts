// Snapping math (pure, unit-tested): grid snap (tokens.grid.step, toggleable) +
// anchor magnetism (jump-to-anchor within radius, returns the connect affordance).

export interface SnapResult {
  x: number;
  y: number;
  anchor?: { shapeId: string; name: string };
}

export function snap(_x: number, _y: number): SnapResult {
  throw new Error("TODO(phase 2): snap");
}
