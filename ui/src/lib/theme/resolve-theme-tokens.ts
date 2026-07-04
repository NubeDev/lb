// Resolve the WIDENED `ctx.theme` payload (`ThemeTokens`) handed to canvas/JS widgets that can't read a
// CSS var (ECharts, three.js) — the theme-inheritance-scope contract, widened by this scope. It reads
// the COMPUTED base tokens off the root (so custom/imported colors are honored, not just presets) as
// concrete `hsl(...)` strings, plus the resolved appearance axes (surface/motion/fonts), the radius, and
// the categorical chart ramp (reused from `features/charts/chartTheme.ts` so extension charts match core
// charts). `features/ext-host` resolves this ONCE per change and fans it out; a DOM consumer needs none
// of it (the cascade re-themes it) — this is for the JS tier.
//
// One responsibility: root computed styles + appearance → ThemeTokens.

import { categoryColors } from "@/features/charts/chartTheme";
import { readComputedBasePalette } from "./read-palette";
import { resolveAppearance } from "./look-resolve";
import { resolveMotion } from "./resolve-motion";
import type { Motion, Surface } from "./appearance-axes";
import type { BaseTokenKey } from "./theme-tokens";
import type { ThemePreference } from "./theme-options";

/** The resolved theme handed to a widget as `ctx.theme` — all concrete strings, no `var()`. Additive
 *  over the theme-inheritance v3 shape: base tokens + new tones + radius + fonts + surface + motion +
 *  the chart ramp. A widget reads what it needs and ignores the rest (forward-compatible). */
export interface ThemeTokens {
  // Base tokens as concrete `hsl(H S% L%)` strings.
  bg: string;
  panel: string;
  fg: string;
  muted: string;
  mutedForeground: string;
  accent: string;
  border: string;
  // Widened tones.
  panel2: string;
  overlay: string;
  accent2: string;
  // Non-color axes.
  radius: string;
  fontSans: string;
  fontMono: string;
  surface: Surface;
  motion: Motion;
  /** The categorical chart ramp (reused from core `chartTheme`) so extension charts match core charts. */
  chart: string[];
}

/** Wrap a computed base-token triplet ("H S% L%") as a concrete `hsl(...)` string; "" stays "". */
function css(triplet: string): string {
  return triplet ? `hsl(${triplet})` : "";
}

/** Resolve `ctx.theme` from the live document + the member's preference. `root`/`doc` are injectable for
 *  tests. Colors come from `getComputedStyle` (honors custom/imported/inline); axes from the resolver. */
export function resolveThemeTokens(
  pref: ThemePreference,
  root: HTMLElement = document.documentElement,
  doc: Document = document,
): ThemeTokens {
  const palette = readComputedBasePalette(root);
  const appearance = resolveAppearance(pref);
  const cs = getComputedStyle(root);
  const asString = (k: BaseTokenKey) => css(palette[k]);

  return {
    bg: asString("bg"),
    panel: asString("panel"),
    fg: asString("fg"),
    muted: asString("muted"),
    mutedForeground: asString("mutedForeground"),
    accent: asString("accent"),
    border: asString("border"),
    panel2: asString("panel2"),
    overlay: asString("overlay"),
    accent2: asString("accent2"),
    radius: cs.getPropertyValue("--radius").trim() || appearance.radius,
    fontSans: cs.getPropertyValue("--font-sans").trim(),
    fontMono: cs.getPropertyValue("--font-mono").trim(),
    surface: appearance.surface,
    motion: resolveMotion(appearance.motion, doc),
    chart: categoryColors(8),
  };
}
