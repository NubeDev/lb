// The sidebar-icon color palette + auto-assignment — the "prefilled 100 colours" the icon-colorizer
// picks from. The palette is DATA, not branches: a deterministic golden-angle hue spread (so any two
// adjacent indices are perceptually far apart), converted to hex once at module load and frozen. The
// auto-assigner evenly samples that palette across whatever surface keys the rail currently shows, so
// 5 icons get 5 evenly-hue-spaced colors (not 5 neighboring reds). Hex validation lives here too —
// it's the single source for "is this a value we’ll store on a surface key".
//
// One responsibility: the icon-color palette + the pure assignment math. No React, no DOM.

import { hslToHex } from "./hsl-triplet";

/** How many colors ship in the prefilled palette. Sized to comfortably cover the rail (currently
 *  ~20 surfaces) with headroom for hand-picks, future extensions, and per-workspace variation. */
export const ICON_COLOR_PALETTE_SIZE = 100;

/** Two saturation/lightness profiles alternate per index — subtle variety while keeping every color
 *  readable on both the light and dark sidebar surfaces (mid lightness + strong saturation). */
function profileFor(i: number): { s: number; l: number } {
  return i % 2 === 0 ? { s: 68, l: 55 } : { s: 62, l: 48 };
}

/** Build the palette deterministically: golden-angle hue spacing (≈137.5°) so consecutive indices are
 *  perceptually far apart, which is what makes a small even sample of the palette look "rainbow-
 *  distributed" rather than clustered. Pure — same output every load. */
function buildPalette(): readonly string[] {
  const out: string[] = [];
  const GOLDEN = 137.508;
  for (let i = 0; i < ICON_COLOR_PALETTE_SIZE; i++) {
    const h = (i * GOLDEN) % 360;
    const { s, l } = profileFor(i);
    out.push(hslToHex({ h, s, l }));
  }
  return Object.freeze(out);
}

/** The prefilled 100-color palette the icon-colorizer picks from. Frozen data — update by editing
 *  `buildPalette`, not by mutating. */
export const ICON_COLOR_PALETTE: readonly string[] = buildPalette();

const HEX_RE = /^#[0-9a-f]{6}$/i;

/** True for a canonical `#rrggbb` hex string. Stored icon colors are kept in this form so the value
 *  is universally editable ("update as needed") and trivially dropped into an inline `style`. */
export function isValidHex(value: unknown): value is string {
  return typeof value === "string" && HEX_RE.test(value);
}

/** Normalize a raw value into a canonical lowercase `#rrggbb`, or `null` if it isn't one. Accepts the
 *  shorthand `#rgb` (expanded) so pasted swatches survive; everything else fails closed to null. */
export function normalizeHex(value: unknown): string | null {
  if (typeof value !== "string") return null;
  const v = value.trim();
  const short = /^#([0-9a-f]{3})$/i.exec(v);
  if (short) {
    const [, c] = short;
    return `#${c[0]}${c[0]}${c[1]}${c[1]}${c[2]}${c[2]}`.toLowerCase();
  }
  return isValidHex(v) ? v.toLowerCase() : null;
}

/** Deterministically assign one palette color to each key, evenly sampling the palette so the result
 *  spans the full hue range no matter how few/many keys there are. Pure and stable — the same keys
 *  always get the same colors, so a re-load doesn't reshuffle the rail. */
export function autoAssignIconColors(keys: readonly string[]): Record<string, string> {
  const n = keys.length;
  if (n === 0) return {};
  const out: Record<string, string> = {};
  for (let i = 0; i < n; i++) {
    // Even sample: for N keys, pick indices 0, 100/N, 2*100/N, … so the rail spans the palette.
    const idx = Math.round((i * ICON_COLOR_PALETTE_SIZE) / n) % ICON_COLOR_PALETTE_SIZE;
    out[keys[i]] = ICON_COLOR_PALETTE[idx];
  }
  return out;
}
