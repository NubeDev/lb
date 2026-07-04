// Read the CURRENTLY-APPLIED base palette off the document root via `getComputedStyle` — the honest
// source of "what colors are on screen right now", whether they came from a preset, a custom theme, or
// the static `:root`/`.dark` blocks. The Brand Colors editor seeds its pickers from this so hand-tweaks
// start from the live values, and the theme-inheritance scope (step 3) resolves `ctx.theme` from it.
// One responsibility: root computed base tokens → BasePalette.

import { BASE_TOKENS, type BasePalette } from "./theme-tokens";

/** Read the base tokens off `root` (default: `document.documentElement`) as "H S% L%" triplet strings.
 *  A missing/blank var yields "" — the caller decides how to handle an incomplete read. */
export function readComputedBasePalette(root: HTMLElement = document.documentElement): BasePalette {
  const cs = getComputedStyle(root);
  const out = {} as BasePalette;
  for (const { key, cssVar } of BASE_TOKENS) {
    out[key] = cs.getPropertyValue(cssVar).trim();
  }
  return out;
}
