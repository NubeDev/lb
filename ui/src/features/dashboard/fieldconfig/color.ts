// Grafana color names → our `ui-standards` token (viz field-config scope, Resolved decision: "Map
// Grafana's semantic color names to our tokens; pass explicit hex through verbatim"). Threshold steps,
// value mappings, and `FieldColor.fixedColor` all carry either a semantic name (`green`/`red`/…) or a
// literal hex; this is the ONE place that resolves a name to a theme-correct CSS color, so dark/light
// + accessibility hold. Never paint a raw named color blind (a renderer must go through `resolveColor`).
//
// One responsibility: name→color resolution. The values are CSS color strings (HSL var refs for the
// themed tokens, so they track the active theme) — a renderer uses them directly as `stroke`/`fill`.

import type { FieldColor } from "@/lib/dashboard";

/** Grafana's semantic palette → our token CSS (theme-tracking via the shipped `--*` HSL vars). A name
 *  not in the map falls through to itself (so a CSS color keyword still works); a `#hex` is verbatim. */
const NAMED: Record<string, string> = {
  green: "hsl(142 71% 45%)",
  red: "hsl(0 72% 51%)",
  orange: "hsl(28 90% 55%)",
  yellow: "hsl(45 93% 47%)",
  blue: "hsl(217 91% 60%)",
  purple: "hsl(270 70% 60%)",
  // semantic → our accent token (theme + accessibility correct)
  accent: "hsl(var(--accent))",
  text: "hsl(var(--fg))",
  "super-light-blue": "hsl(217 91% 80%)",
  "dark-red": "hsl(0 72% 38%)",
};

/** Resolve a Grafana color string (semantic name OR literal hex/CSS) to a CSS color. A literal
 *  `#rrggbb`/`rgb()`/`hsl()` passes through verbatim; a known semantic name maps to a theme token; an
 *  unknown name falls through as-is (a valid CSS keyword still renders). */
export function resolveColor(color: string | undefined, fallback = "hsl(var(--accent))"): string {
  if (!color) return fallback;
  const c = color.trim();
  if (c.startsWith("#") || c.startsWith("rgb") || c.startsWith("hsl")) return c;
  return NAMED[c] ?? c;
}

/** Resolve a `FieldColor` to a single line/fill color for the simple modes a Phase-1 timeseries uses
 *  (`fixed` → its color; everything else → the accent token, with thresholds handled per-point by
 *  `thresholds.ts`). Continuous/palette modes resolve to the accent token until their phase. */
export function resolveFieldColor(color: FieldColor | undefined, fallback = "hsl(var(--accent))"): string {
  if (!color) return fallback;
  if (color.mode === "fixed") return resolveColor(color.fixedColor, fallback);
  return fallback;
}
