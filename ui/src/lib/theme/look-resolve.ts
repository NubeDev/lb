// The look resolver — the pure fold that turns a ThemePreference into a concrete, applyable
// ResolvedAppearance. Per-axis precedence, mirroring the prefs resolve chain the project already teaches:
//
//   pinned look axis  →  explicit member override  →  look default  →  built-in default
//
// A look DEFAULTS the axes it defines (a member override wins), except a look's `pins` axes, which the
// look OWNS (they win even over a member override — only `retro` pins its preset, because its identity
// IS the palette). An unknown look id falls to `default` (fail-open to data). Kept pure + separate from
// `theme-dom` so the DOM layer just writes what it's handed and this is unit-testable without a document.
//
// One responsibility: ThemePreference → ResolvedAppearance.

import { DEFAULT_MOTION, DEFAULT_SURFACE, type Motion, type Surface } from "./appearance-axes";
import { DEFAULT_FONT_MONO, DEFAULT_FONT_SANS } from "./theme-fonts.data";
import { DEFAULT_LOOK, lookById, type LookDefaults } from "./theme-looks.data";
import type { ThemePreference, ThemeRadius } from "./theme-options";

/** The concrete appearance every consumer applies — no undefined, every axis decided. */
export interface ResolvedAppearance {
  look: string;
  preset: string;
  radius: ThemeRadius;
  fontSans: string;
  fontMono: string;
  surface: Surface;
  motion: Motion;
}

/** Fold `pref` into a concrete appearance. `preset`/`radius` are required on the preference (always
 *  present), but a look still pins them where it declares so; the optional axes fall through the chain. */
export function resolveAppearance(pref: ThemePreference): ResolvedAppearance {
  const look = lookById(pref.look) ?? lookById(DEFAULT_LOOK);
  const d: LookDefaults = look?.defaults ?? {};
  const pins = new Set(look?.pins ?? []);

  /** Fold one axis: a pinned look value wins over the member; otherwise member → look default → builtin. */
  const pick = <K extends keyof LookDefaults>(axis: K, member: unknown, builtin: NonNullable<LookDefaults[K]>): NonNullable<LookDefaults[K]> => {
    if (pins.has(axis) && d[axis] != null) return d[axis] as NonNullable<LookDefaults[K]>;
    if (member != null) return member as NonNullable<LookDefaults[K]>;
    if (d[axis] != null) return d[axis] as NonNullable<LookDefaults[K]>;
    return builtin;
  };

  return {
    look: look?.id ?? DEFAULT_LOOK,
    // preset/radius are always set on the preference, so they ARE the member value (unless pinned).
    preset: pick("preset", pref.preset, pref.preset),
    radius: pick("radius", pref.radius, pref.radius),
    fontSans: pick("fontSans", pref.fontSans, DEFAULT_FONT_SANS),
    fontMono: pick("fontMono", pref.fontMono, DEFAULT_FONT_MONO),
    surface: pick("surface", pref.surface, DEFAULT_SURFACE),
    motion: pick("motion", pref.motion, DEFAULT_MOTION),
  };
}

/** The preference produced by PICKING a look: reset the axes the look defines (drop member overrides so
 *  the look lands like its thumbnail), stamp the look's preset/radius, keep custom/imported cleared by
 *  the caller. Axes the look doesn't define fall to the built-in default. */
export function applyLook(pref: ThemePreference, lookId: string): ThemePreference {
  const look = lookById(lookId) ?? lookById(DEFAULT_LOOK)!;
  const d = look.defaults;
  return {
    ...pref,
    look: look.id,
    preset: d.preset ?? pref.preset,
    radius: d.radius ?? pref.radius,
    // Reset the per-axis overrides to inherit the freshly-picked look's defaults.
    fontSans: undefined,
    fontMono: undefined,
    surface: undefined,
    motion: undefined,
  };
}
