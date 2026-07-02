// type → symbol component dispatch, plus the three cross-cutting behaviors every
// shape gets for free: the labeled PLACEHOLDER box for unknown types (never a crash —
// symbols-scope.md §placeholder), the cyan selection halo, and bound-value resolution
// (subscribes shape.bind channels via the ValueSource seam, passes values down).

import type { SceneShape } from "../scene/scene.types";

export function ShapeNode(_props: { id: string; shape: SceneShape }) {
  // TODO(phase 1): registry lookup (shapes/*), useValues(shape.bind), selection halo,
  // placeholder fallback.
  return null;
}
