// Canvas layout helpers (pure) — kept out of the React node-builder so the
// position math can be tested directly.

export const STACK_OFFSET = 16; // diagonal nudge per overlapping duplicate

interface Positioned {
  metadata?: { position?: { x?: number; y?: number } };
}

// Resolve each component's on-canvas position:
//   - If EVERY component sits at (0,0) — the engine hasn't laid them out yet —
//     fall back to a square grid.
//   - Otherwise use the saved positions, but de-stack exact duplicates: any
//     component sharing an (x,y) already used gets nudged diagonally so a pile of
//     same-position nodes reads as a stack of cards instead of one node. Display
//     only — deterministic in input order, so a reload reproduces it exactly.
export function layoutPositions(
  comps: Positioned[],
  nodeWidth: number,
): { x: number; y: number }[] {
  const allZero = comps.every(
    (c) => (c.metadata?.position?.x ?? 0) === 0 && (c.metadata?.position?.y ?? 0) === 0,
  );
  const cols = Math.max(1, Math.ceil(Math.sqrt(comps.length)));
  const GRID_X = nodeWidth + 60;
  const GRID_Y = 220;
  const stackSeen = new Map<string, number>();
  return comps.map((c, i) => {
    const px = c.metadata?.position?.x ?? 0;
    const py = c.metadata?.position?.y ?? 0;
    if (allZero) {
      return { x: (i % cols) * GRID_X, y: Math.floor(i / cols) * GRID_Y };
    }
    const key = `${px},${py}`;
    const dup = stackSeen.get(key) ?? 0;
    stackSeen.set(key, dup + 1);
    return { x: px + dup * STACK_OFFSET, y: py + dup * STACK_OFFSET };
  });
}
