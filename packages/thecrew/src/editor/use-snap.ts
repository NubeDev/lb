// Snapping math (pure, unit-tested): grid snap (tokens.grid.step, toggleable) +
// anchor magnetism (jump-to-anchor within radius, returns the connect affordance).
// Anchor snap WINS over grid snap — placing "onto" an anchor is place-and-connect
// in one gesture (builder-ux-scope.md §place).

import { tokens } from "../theme/tokens";

export interface WorldAnchor {
  shapeId: string;
  name: string;
  x: number;
  y: number;
}

export interface SnapResult {
  x: number;
  y: number;
  /** present when anchor magnetism captured the point — the connect affordance */
  anchor?: { shapeId: string; name: string };
}

export interface SnapOptions {
  /** grid step; 0/undefined disables grid snap (toolbar toggle off) */
  grid?: number;
  /** world-space anchors eligible for magnetism */
  anchors?: WorldAnchor[];
  /** capture radius for anchor magnetism, world units */
  anchorRadius?: number;
}

export const DEFAULT_ANCHOR_RADIUS = tokens.grid.step * 2;

export function snapToGrid(v: number, step: number): number {
  return Math.round(v / step) * step;
}

export function snap(x: number, y: number, opts: SnapOptions = {}): SnapResult {
  const radius = opts.anchorRadius ?? DEFAULT_ANCHOR_RADIUS;

  // 1. anchor magnetism first — nearest anchor within radius wins outright
  let best: WorldAnchor | undefined;
  let bestD2 = radius * radius;
  for (const a of opts.anchors ?? []) {
    const dx = a.x - x;
    const dy = a.y - y;
    const d2 = dx * dx + dy * dy;
    if (d2 <= bestD2) {
      bestD2 = d2;
      best = a;
    }
  }
  if (best) {
    return { x: best.x, y: best.y, anchor: { shapeId: best.shapeId, name: best.name } };
  }

  // 2. grid snap
  if (opts.grid && opts.grid > 0) {
    return { x: snapToGrid(x, opts.grid), y: snapToGrid(y, opts.grid) };
  }
  return { x, y };
}
