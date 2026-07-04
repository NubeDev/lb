// Derive the WIDENED tones from the required seven — the compatibility fold at the heart of the tone
// widening (theme-appearance scope). New palette tokens (`panel2`, `overlay`, `accent2`) join the base
// set, but a stored v1 custom theme only carries the seven. Rather than fail-closed and DROP a member's
// hand-tuned palette (the blocker the scope calls out), we DERIVE the missing tones from the seven the
// same relative way the static `:root`/`.dark` blocks do: a raised panel is `--panel` nudged toward the
// foreground, an overlay is `--bg` nudged the same way, a secondary accent is `--accent` shifted in hue.
// Mode matters: "toward foreground" is lighter in dark mode, darker in light mode.
//
// One responsibility: required-seven palette → the derived widened tones.

import { formatTriplet, parseTriplet, type Hsl } from "./hsl-triplet";
import type { DerivedTones, RequiredPalette } from "./theme-tokens";

/** Nudge a triplet's lightness by `delta` points, clamped by `formatTriplet`. */
function lighten(triplet: string, delta: number): string {
  const hsl = parseTriplet(triplet);
  if (!hsl) return triplet;
  return formatTriplet({ ...hsl, l: hsl.l + delta });
}

/** Rotate a triplet's hue by `deg` degrees (for a secondary accent that's related but distinct). */
function rotateHue(triplet: string, deg: number): string {
  const hsl = parseTriplet(triplet);
  if (!hsl) return triplet;
  return formatTriplet({ ...hsl, h: hsl.h + deg } as Hsl);
}

/** Is this palette a dark one? Foreground lighter than background ⇒ dark mode. Defaults to dark on an
 *  unparseable pair (the shipped default is dark). */
function isDark(p: RequiredPalette): boolean {
  const bg = parseTriplet(p.bg);
  const fg = parseTriplet(p.fg);
  if (!bg || !fg) return true;
  return fg.l > bg.l;
}

/** Derive the three widened tones from the required seven. A raised surface steps ~3–4pts toward the
 *  foreground; the OVERLAY is a scrim tone (a near-black derived from the background's hue, dark in BOTH
 *  modes so a modal backdrop always darkens content behind it); the secondary accent rotates the accent
 *  hue by +40° (a complementary-ish sibling). */
export function deriveTones(p: RequiredPalette): DerivedTones {
  const toward = isDark(p) ? +1 : -1; // toward foreground
  return {
    panel2: lighten(p.panel, 4 * toward),
    overlay: overlayScrim(p.bg),
    accent2: rotateHue(p.accent, 40),
  };
}

/** A scrim tone: the background's hue at a low, dark lightness so modal backdrops darken in either mode. */
function overlayScrim(bg: string): string {
  const hsl = parseTriplet(bg);
  if (!hsl) return "0 0% 6%";
  return formatTriplet({ ...hsl, s: Math.min(hsl.s, 20), l: 8 });
}

/** Fill any absent widened tone on a partial palette from the derivation, keeping present ones. Used by
 *  the normalizer so a v1 stored theme (seven tokens) upgrades to the full set without dropping. */
export function completeTones(p: RequiredPalette, partial?: Partial<DerivedTones>): DerivedTones {
  const derived = deriveTones(p);
  return {
    panel2: nonEmpty(partial?.panel2) ?? derived.panel2,
    overlay: nonEmpty(partial?.overlay) ?? derived.overlay,
    accent2: nonEmpty(partial?.accent2) ?? derived.accent2,
  };
}

function nonEmpty(v: unknown): string | undefined {
  return typeof v === "string" && v.trim().length > 0 ? v : undefined;
}
