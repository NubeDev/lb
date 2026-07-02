// Per-symbol-type defaults: what the palette drops onto the canvas
// (builder-ux-scope.md §place). One entry per type in symbols-scope.md's tables.

import type { SceneShape } from "./scene.types";

export function defaultShape(_type: string): SceneShape {
  // TODO(phase 2): default transform/props per symbol type (e.g. hvac.fan diameter,
  // plan.wall thickness), so a fresh drop always renders sensibly.
  throw new Error("TODO(phase 2): defaultShape");
}
