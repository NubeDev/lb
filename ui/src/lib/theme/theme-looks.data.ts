// The look packs — DATA, not branches (FILE-LAYOUT). A "look" is a one-click bundle of DEFAULTS across
// every appearance axis (color preset, fonts, radius, surface, motion). Picking a look resets the axes
// it defines; an explicit member override still wins per-axis (see `look-resolve.ts`). Adding a look is
// adding a row here — ZERO code branches (rule 8/10 in spirit: the resolver treats a look id as opaque).
//
// `pins` lists axes a look OWNS rather than merely defaults: a pinned axis is applied even over an
// explicit member choice, because the look's identity IS that axis. Only `retro` pins (its palette is
// the look) — expressed as data (`pins: ["preset"]`), still no branch.
//
// One responsibility: the look catalog + its per-axis defaults.

import type { Surface, Motion, GlassLevel } from "./appearance-axes";
import type { ThemeMode, ThemeRadius } from "./theme-options";

/** The axes a look can default. Every field optional — a look defines only what makes it that look;
 *  undefined axes fall through to the built-in default in the resolver. `mode` (light/dark) is part of a
 *  look's identity — Professional is a *paper* look, retro/glass are dark-forward — so a look stamps it
 *  on pick (`applyLook`); the member can still flip the mode toggle afterward. */
export interface LookDefaults {
  mode?: ThemeMode;
  preset?: string;
  fontSans?: string;
  fontMono?: string;
  radius?: ThemeRadius;
  surface?: Surface;
  motion?: Motion;
  /** Glass intensity a look lands at (only read when its `surface` is `glass`). */
  glass?: GlassLevel;
}

export interface LookPack {
  id: string;
  label: string;
  /** One-line description shown on the picker card. */
  blurb: string;
  defaults: LookDefaults;
  /** Axes this look OWNS (applied even over an explicit member choice). Empty for all but `retro`. */
  pins?: ReadonlyArray<keyof LookDefaults>;
}

export const THEME_LOOKS: readonly LookPack[] = [
  {
    id: "default",
    label: "Operator Console",
    blurb: "The shipped teal console — dark-first, calm, sharp.",
    defaults: { mode: "dark", preset: "teal", fontSans: "system", fontMono: "system-mono", radius: "0.5rem", surface: "flat", motion: "subtle" },
  },
  {
    id: "editor",
    label: "Code Editor",
    blurb: "Dark, dense, sharp-cornered — mono-forward with a muted syntax palette.",
    // Editor is a code surface: near-black slate, sharp corners, IBM Plex Sans chrome + JetBrains Mono.
    defaults: { mode: "dark", preset: "editor", fontSans: "ibm-plex-sans", fontMono: "jetbrains-mono", radius: "0.3rem", surface: "flat", motion: "subtle" },
  },
  {
    id: "professional",
    label: "Professional",
    blurb: "Light paper, a serif voice, generous space and real elevation.",
    // A genuinely LIGHT look — paper background, serif headings, elevated cards. Mode is stamped light.
    defaults: { mode: "light", preset: "slate", fontSans: "source-serif-4", fontMono: "ibm-plex-mono", radius: "0.5rem", surface: "elevated", motion: "subtle" },
  },
  {
    id: "retro",
    label: "Retro Terminal",
    blurb: "Amber phosphor on true black, square corners, JetBrains Mono.",
    defaults: { mode: "dark", preset: "retro", fontSans: "jetbrains-mono", fontMono: "jetbrains-mono", radius: "0rem", surface: "flat", motion: "subtle" },
    // Retro's identity IS its palette — pin the preset so it survives even an explicit member preset.
    pins: ["preset"],
  },
  {
    id: "modern",
    label: "Modern Dashboard",
    blurb: "Airy light dashboard — large radius, soft gradients, elevated cards.",
    // Modern is an airy LIGHT dashboard: big radius, visible soft gradient, floating cards. Mode: light.
    defaults: { mode: "light", preset: "ocean", fontSans: "geist", fontMono: "ibm-plex-mono", radius: "1rem", surface: "elevated", motion: "full" },
  },
  {
    id: "glass",
    label: "Liquid Glass",
    blurb: "Translucent panels, backdrop blur, layered elevation, gradient accents.",
    // Lands at `medium` glass — the balanced translucency that reads as the look's identity; a member
    // can dial it to subtle/heavy via the glass-intensity control (only shown when the surface is glass).
    defaults: { mode: "dark", preset: "violet", fontSans: "inter", fontMono: "jetbrains-mono", radius: "0.75rem", surface: "glass", motion: "full", glass: "medium" },
  },
] as const;

export const DEFAULT_LOOK = "default";

export function lookById(id: string): LookPack | undefined {
  return THEME_LOOKS.find((l) => l.id === id);
}
