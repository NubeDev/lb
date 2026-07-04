// WCAG contrast ratio between two HSL-triplet colors — used by the AA test that vets every shipped look
// in both modes (theme-appearance scope: shipped looks are contrast-vetted, imports stay warn-only).
// Ratio = (L1 + 0.05) / (L2 + 0.05) on relative luminance; AA normal text needs ≥ 4.5, AA large ≥ 3.
//
// One responsibility: (triplet, triplet) → contrast ratio.

import { hslToHex, parseTriplet } from "./hsl-triplet";

/** Relative luminance (WCAG) of a `#rrggbb` color. */
function luminance(hex: string): number {
  const to = (c: number) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
  };
  const r = to(parseInt(hex.slice(1, 3), 16));
  const g = to(parseInt(hex.slice(3, 5), 16));
  const b = to(parseInt(hex.slice(5, 7), 16));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** Contrast ratio between two HSL triplets (order-independent), 1..21. Returns 1 on an unparseable pair. */
export function contrastRatio(a: string, b: string): number {
  const ha = parseTriplet(a);
  const hb = parseTriplet(b);
  if (!ha || !hb) return 1;
  const la = luminance(hslToHex(ha));
  const lb = luminance(hslToHex(hb));
  const [hi, lo] = la >= lb ? [la, lb] : [lb, la];
  return (hi + 0.05) / (lo + 0.05);
}

/** WCAG AA for normal body text. */
export const AA_NORMAL = 4.5;
/** WCAG AA for large/UI text (≥18pt or 14pt bold). */
export const AA_LARGE = 3;
