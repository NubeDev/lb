// Fit the ortho camera to the scene + the viewport (thecrew parent-scope risk: "WebGL contexts / fit
// per dashboard cell"). The editor page frames a fixed ±350 world units at a fixed zoom, which is right
// for a large page but leaves a SMALL dashboard cell showing only a tiny center crop — the scene reads
// blank when its shapes sit away from the origin. A read-only cell instead auto-fits: frame the scene's
// bounding box into the actual cell pixels. Pure math (no three import) so it unit-tests without a GL
// context; the camera-applying wrapper lives in `FitCamera.tsx`.

import type { SceneDoc } from "../scene/scene.types";

export interface Bounds {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
}

/** The scene's XY bounding box from shape transforms. Empty scene → a small box around the origin so
 *  fit stays well-defined (no divide-by-zero). Ignores z (flat fit); scale isn't known here so each
 *  shape counts as a point — the padding factor below absorbs finite shape extent. */
export function sceneBounds(doc: SceneDoc): Bounds {
  const xs: number[] = [];
  const ys: number[] = [];
  for (const s of Object.values(doc.shapes)) {
    xs.push(s.t.x);
    ys.push(s.t.y);
  }
  if (xs.length === 0) return { minX: -100, minY: -100, maxX: 100, maxY: 100 };
  return {
    minX: Math.min(...xs),
    minY: Math.min(...ys),
    maxX: Math.max(...xs),
    maxY: Math.max(...ys),
  };
}

/** The ortho `zoom` that frames `bounds` (world units) into a `width`×`height` px viewport, with `pad`
 *  world units of margin on every side (default 120 — roughly one large symbol, so equipment near an
 *  edge isn't clipped). drei's OrthographicCamera maps world→px by `zoom`, so the visible world span is
 *  `px / zoom`; we pick the tighter axis so the whole box fits, and clamp to the page's pan/zoom limits
 *  ([0.4, 6]) so a tiny or huge scene stays usable. A zero-area viewport (pre-layout) yields the page
 *  default 1.6 rather than Infinity. */
export function fitZoom(
  bounds: Bounds,
  width: number,
  height: number,
  pad = 120,
): number {
  if (width <= 0 || height <= 0) return 1.6;
  const spanX = bounds.maxX - bounds.minX + pad * 2;
  const spanY = bounds.maxY - bounds.minY + pad * 2;
  const zx = width / Math.max(spanX, 1);
  const zy = height / Math.max(spanY, 1);
  const z = Math.min(zx, zy);
  return Math.max(0.4, Math.min(6, z));
}

/** The world-space center of `bounds` — the ortho camera's XY target so the box is centered in the cell
 *  (the scene isn't assumed to sit at the origin). */
export function boundsCenter(bounds: Bounds): { x: number; y: number } {
  return { x: (bounds.minX + bounds.maxX) / 2, y: (bounds.minY + bounds.maxY) / 2 };
}
