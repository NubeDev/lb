// The one place chart chrome reads the theme — axis/grid/tooltip styling and the categorical series
// palette, all off the app's HSL design tokens (`styles/globals.css`) so charts track light/dark and
// the accent hue automatically. Centralising it here is why every chart on both surfaces looks like
// one system instead of the old per-file hand-drawn axes that drifted.
//
// One responsibility: chart style tokens. No React, no data.

/** A curated categorical ramp for multi-series charts — eight well-separated hues that stay legible on
 *  both the warm-sand light surface and the charcoal dark surface. A SINGLE-series chart uses the brand
 *  accent instead (see `seriesColor`) so a lone line/bar reads as "the app", not a random hue. */
const PALETTE = [
  // Token-bound (`--chart-N`, globals.css) so the ramp is tuned PER MODE (brighter cuts on dark,
  // deeper cuts on paper) and any preset/theme can re-voice it — the old literal hsl() values were
  // the reason multi-series charts ignored the theme entirely.
  "hsl(var(--chart-1))", // violet
  "hsl(var(--chart-2))", // teal
  "hsl(var(--chart-3))", // orange
  "hsl(var(--chart-4))", // cyan
  "hsl(var(--chart-5))", // rose
  "hsl(var(--chart-6))", // green
  "hsl(var(--chart-7))", // gold
  "hsl(var(--chart-8))", // blue
] as const;

/** The color for series `index` of `total`. One series → the accent token (brand cohesion); many →
 *  the categorical ramp, wrapping if there are more series than hues. */
export function seriesColor(index: number, total: number): string {
  if (total <= 1) return "hsl(var(--accent))";
  return PALETTE[index % PALETTE.length];
}

/** The full ramp, for a pie/bar that colors per category rather than per series. */
export function categoryColors(count: number): string[] {
  if (count <= 1) return ["hsl(var(--accent))"];
  return Array.from({ length: count }, (_, i) => PALETTE[i % PALETTE.length]);
}

/** Axis tick + label styling — muted, small, tabular so numbers align. */
export const axisTick = { fontSize: 11, fill: "hsl(var(--muted))" } as const;
export const axisLine = { stroke: "hsl(var(--border))" } as const;
export const axisLabelStyle = {
  fontSize: 11,
  fill: "hsl(var(--muted-foreground, var(--muted)))",
  fontWeight: 500,
  letterSpacing: "0.02em",
} as const;

/** Gridlines — faint, dashed, so they guide the eye without competing with the data. */
export const gridStroke = "hsl(var(--border))";
export const gridProps = { stroke: gridStroke, strokeDasharray: "3 6", strokeOpacity: 0.4 } as const;

/** The themed tooltip surface (matches popover tokens). */
export const tooltipSurface = {
  background: "hsl(var(--popover))",
  border: "1px solid hsl(var(--border))",
  borderRadius: 8,
  boxShadow: "0 8px 24px -8px hsl(var(--fg) / 0.25)",
  fontSize: 12,
  color: "hsl(var(--fg))",
  padding: "8px 10px",
} as const;
