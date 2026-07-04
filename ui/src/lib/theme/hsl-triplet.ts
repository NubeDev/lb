// Parse/format the project's base-token color format — the bare HSL channel triplet "H S% L%" consumed
// as `hsl(var(--…))`. The Brand Colors popover picker edits H/S/L directly and shows a hex field, so it
// needs to go triplet ⇄ {h,s,l} ⇄ hex in both directions. `color-to-hsl.ts` already does hex→triplet
// (and oklch/hsl-fn→triplet); this file owns the triplet's own (de)serialization + the triplet→hex leg,
// kept here (not in the picker) so the value math is unit-testable without a DOM.
//
// One responsibility: the "H S% L%" triplet's parse/format/hex round-trip.

export interface Hsl {
  h: number;
  s: number;
  l: number;
}

/** Parse "H S% L%" (the token format) into channels, or null if it isn't a well-formed triplet. */
export function parseTriplet(triplet: string): Hsl | null {
  const m = triplet.trim().match(/^(-?\d+(?:\.\d+)?)\s+(-?\d+(?:\.\d+)?)%\s+(-?\d+(?:\.\d+)?)%$/);
  if (!m) return null;
  return { h: parseFloat(m[1]), s: parseFloat(m[2]), l: parseFloat(m[3]) };
}

/** Format channels back into the token triplet, clamped to valid ranges and rounded to integers. */
export function formatTriplet({ h, s, l }: Hsl): string {
  const H = ((Math.round(h) % 360) + 360) % 360;
  const S = clamp(Math.round(s), 0, 100);
  const L = clamp(Math.round(l), 0, 100);
  return `${H} ${S}% ${L}%`;
}

/** HSL channels → `#rrggbb`. Used for the picker's hex field + the swatch preview. */
export function hslToHex({ h, s, l }: Hsl): string {
  const sn = clamp(s, 0, 100) / 100;
  const ln = clamp(l, 0, 100) / 100;
  const c = (1 - Math.abs(2 * ln - 1)) * sn;
  const hp = (((h % 360) + 360) % 360) / 60;
  const x = c * (1 - Math.abs((hp % 2) - 1));
  const [r1, g1, b1] =
    hp < 1 ? [c, x, 0] : hp < 2 ? [x, c, 0] : hp < 3 ? [0, c, x] : hp < 4 ? [0, x, c] : hp < 5 ? [x, 0, c] : [c, 0, x];
  const m = ln - c / 2;
  const to = (n: number) =>
    Math.round((n + m) * 255)
      .toString(16)
      .padStart(2, "0");
  return `#${to(r1)}${to(g1)}${to(b1)}`;
}

/** The triplet as a ready-to-use CSS `hsl(...)` string (for a swatch background, no `var()`). */
export function tripletToCss(triplet: string): string {
  const hsl = parseTriplet(triplet);
  return hsl ? `hsl(${formatTriplet(hsl).replace(/%/g, "%")})` : "transparent";
}

function clamp(n: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, n));
}
