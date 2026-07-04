// The appearance axes the look/font/surface/motion widening adds to a ThemePreference — their value
// vocabularies, defaults, and type guards. Kept separate from `theme-options.ts` (which owns the
// preference shape) so the axis enums have one home the pickers, the resolver, and the DOM layer share.
//
// One responsibility: the surface/motion axis vocabularies + guards. (Font ids live in
// `theme-fonts.data.ts`; look ids in `theme-looks.data.ts` — both data, not enums.)

/** Surface treatment — how panels/cards/sheets/nav render. `flat` is today; `elevated` adds a shadow
 *  ramp; `glass` adds translucency + backdrop blur (degrades to elevated/flat where blur is weak). */
export const SURFACES = ["flat", "elevated", "glass"] as const;
export type Surface = (typeof SURFACES)[number];

/** Motion profile — how much interaction motion the shell chrome uses. `off` fences all transitions
 *  (also forced by `prefers-reduced-motion` unless the member explicitly chose `full`); `subtle` is
 *  short, small-distance variants; `full` is the designed set. */
export const MOTIONS = ["off", "subtle", "full"] as const;
export type Motion = (typeof MOTIONS)[number];

export const DEFAULT_SURFACE: Surface = "flat";
/** New members default to `subtle` — respect first impressions + low-end hardware; `full` is a choice. */
export const DEFAULT_MOTION: Motion = "subtle";

export function isSurface(v: unknown): v is Surface {
  return typeof v === "string" && (SURFACES as readonly string[]).includes(v);
}
export function isMotion(v: unknown): v is Motion {
  return typeof v === "string" && (MOTIONS as readonly string[]).includes(v);
}

export const SURFACE_OPTIONS: ReadonlyArray<{ value: Surface; label: string; hint: string }> = [
  { value: "flat", label: "Flat", hint: "Solid panels, no elevation" },
  { value: "elevated", label: "Elevated", hint: "Opaque panels with a shadow ramp" },
  { value: "glass", label: "Liquid glass", hint: "Translucent panels with backdrop blur" },
];

export const MOTION_OPTIONS: ReadonlyArray<{ value: Motion; label: string; hint: string }> = [
  { value: "off", label: "Off", hint: "No interaction motion" },
  { value: "subtle", label: "Subtle", hint: "Short, small transitions" },
  { value: "full", label: "Full", hint: "The designed springy set" },
];
