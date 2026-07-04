// Resolve the EFFECTIVE motion level, honoring the OS `prefers-reduced-motion` setting. The rule (scope):
// reduced-motion forces `off` UNLESS the member explicitly chose `full` — an explicit `full` is treated
// as an informed opt-in that overrides the OS hint; `subtle`/`off` collapse to `off` under reduced
// motion. This is the single place that fold lives so both `data-motion` (theme-dom) and `useMotionPref`
// (the JS seam) agree.
//
// One responsibility: (member motion, environment) → effective motion attribute value.

import type { Motion } from "./appearance-axes";

/** True when the environment asks for reduced motion. Guards `matchMedia` for jsdom/SSR (absent → false). */
export function prefersReducedMotion(doc: Document = document): boolean {
  const view = doc.defaultView;
  if (!view || typeof view.matchMedia !== "function") return false;
  return view.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/** The motion value to apply. `full` overrides the OS hint (explicit opt-in); everything else is forced
 *  to `off` when the OS asks for reduced motion. */
export function resolveMotion(motion: Motion, doc: Document = document): Motion {
  if (motion === "full") return "full";
  return prefersReducedMotion(doc) ? "off" : motion;
}
