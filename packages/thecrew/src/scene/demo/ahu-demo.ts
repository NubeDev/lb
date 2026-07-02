// The seeded AHU-1 demo scene — the look-scope hero shot (phase 1 exit gate).
// Composes exactly the six hvac.* symbols: casing, OA damper, filter, coil, fan,
// duct run — bindings onto simulator channels (data/simulator.ts).

import type { SceneDoc } from "../scene.types";

export const ahuDemo: SceneDoc = {
  v: 1,
  camera: "ortho-top",
  shapes: {
    // TODO(phase 1): the full AHU-1 composition per symbols-scope.md family 1.
  },
};
