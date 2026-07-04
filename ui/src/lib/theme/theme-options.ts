// The theme preference shape + validation — the single source of truth for what a member's theme
// *is*. Widened from the shipped `{mode, accent}` to the Customizer's full surface
// `{mode, preset, radius, custom?, imported?}`. NO backward-compat with the old shape: `accent` is
// gone (its three values are three built-in presets), and a legacy/malformed stored value normalizes
// straight to DEFAULT_THEME rather than being upgraded. One responsibility: the preference type + its
// validation. Token *values* live in `theme-tokens.ts`; preset packs are data in `theme-presets.data.ts`.

import type { CustomTheme } from "./theme-tokens";
import { isCustomTheme } from "./theme-tokens";
import { completeCustomTheme } from "./normalize-custom-theme";
import { isSurface, isMotion, isGlass, type Surface, type Motion, type GlassLevel } from "./appearance-axes";
import { fontById } from "./theme-fonts.data";
import { lookById } from "./theme-looks.data";

export const THEME_MODES = ["dark", "light"] as const;
export type ThemeMode = (typeof THEME_MODES)[number];

/** The five corner-radius stops the Customizer exposes; each is a CSS length written to `--radius`. */
export const THEME_RADII = ["0rem", "0.3rem", "0.5rem", "0.75rem", "1rem"] as const;
export type ThemeRadius = (typeof THEME_RADII)[number];

/** The three built-in accent presets that shipped with the theme switcher, now first-class presets.
 *  They apply via the `data-theme-accent` attribute (their values live in `globals.css`), so they are
 *  the ONE preset family that needs no inline base-token write. Any other preset id is a library/
 *  imported preset resolved through the adapter. */
export const BUILTIN_PRESETS = ["amber", "teal", "blue"] as const;
export type BuiltinPreset = (typeof BUILTIN_PRESETS)[number];

/** The sidebar layout axes — mapped 1:1 onto the shipped shadcn `<Sidebar>` props (`sidebar.tsx`).
 *  These are shell chrome, not colors, but they ride the same theme preference so they persist and
 *  roam through the one `ui_theme` blob. */
export const SIDEBAR_VARIANTS = ["sidebar", "floating", "inset"] as const;
export const SIDEBAR_COLLAPSIBLES = ["offcanvas", "icon", "none"] as const;
export const SIDEBAR_SIDES = ["left", "right"] as const;

export type SidebarVariant = (typeof SIDEBAR_VARIANTS)[number];
export type SidebarCollapsible = (typeof SIDEBAR_COLLAPSIBLES)[number];
export type SidebarSide = (typeof SIDEBAR_SIDES)[number];

export interface ThemeLayout {
  variant: SidebarVariant;
  collapsible: SidebarCollapsible;
  side: SidebarSide;
}

export const DEFAULT_LAYOUT: ThemeLayout = {
  variant: "sidebar",
  collapsible: "icon",
  side: "left",
};

/** A member's full theme preference. `preset` is a built-in id OR a library preset id; `custom` holds
 *  per-token brand overrides (both modes); `imported` holds a pasted tweakcn theme parsed to base
 *  tokens. When `custom`/`imported` is present it takes precedence over `preset` in application.
 *  `layout` is the sidebar variant/collapsible/side (Layout tab).
 *
 *  APPEARANCE AXES (theme-appearance scope). `look` is a one-click look-pack id whose DEFAULTS fill any
 *  axis the member hasn't explicitly set. `preset`/`radius` and the OPTIONAL `fontSans`/`fontMono`/
 *  `surface`/`motion` are per-axis overrides: an explicit value wins over the look default (except a
 *  look's *pinned* axes, which win over the member — see `look-resolve.ts`). Leaving an axis undefined
 *  = "inherit from the look". Picking a look resets the axes it defines (the provider clears the
 *  overrides), so a look lands looking like its thumbnail. `preset`/`radius` stay required (a value is
 *  always present) but a fresh look pick overwrites them with the look's defaults. */
export interface ThemePreference {
  mode: ThemeMode;
  preset: string;
  radius: ThemeRadius;
  layout: ThemeLayout;
  look: string;
  fontSans?: string;
  fontMono?: string;
  surface?: Surface;
  motion?: Motion;
  /** Glass intensity override (only meaningful under `surface==="glass"`). Undefined = inherit the look. */
  glass?: GlassLevel;
  custom?: CustomTheme;
  imported?: CustomTheme;
}

export const DEFAULT_THEME: ThemePreference = {
  mode: "dark",
  preset: "amber",
  radius: "0.5rem",
  layout: DEFAULT_LAYOUT,
  look: "default",
};

export const THEME_STORAGE_KEY = "lb.theme";

export const THEME_MODE_OPTIONS: ReadonlyArray<{ value: ThemeMode; label: string }> = [
  { value: "dark", label: "Dark" },
  { value: "light", label: "Light" },
];

export const THEME_RADIUS_OPTIONS: ReadonlyArray<{ value: ThemeRadius; label: string }> = [
  { value: "0rem", label: "0" },
  { value: "0.3rem", label: "0.3" },
  { value: "0.5rem", label: "0.5" },
  { value: "0.75rem", label: "0.75" },
  { value: "1rem", label: "1" },
];

export function isThemeMode(value: unknown): value is ThemeMode {
  return typeof value === "string" && THEME_MODES.includes(value as ThemeMode);
}

export function isThemeRadius(value: unknown): value is ThemeRadius {
  return typeof value === "string" && THEME_RADII.includes(value as ThemeRadius);
}

export function isBuiltinPreset(value: unknown): value is BuiltinPreset {
  return typeof value === "string" && BUILTIN_PRESETS.includes(value as BuiltinPreset);
}

/** Validate a layout blob, filling any unknown/absent axis from DEFAULT_LAYOUT (never partial). */
function normalizeLayout(value: unknown): ThemeLayout {
  const c = (value && typeof value === "object" ? value : {}) as Partial<ThemeLayout>;
  return {
    variant: SIDEBAR_VARIANTS.includes(c.variant as SidebarVariant) ? (c.variant as SidebarVariant) : DEFAULT_LAYOUT.variant,
    collapsible: SIDEBAR_COLLAPSIBLES.includes(c.collapsible as SidebarCollapsible)
      ? (c.collapsible as SidebarCollapsible)
      : DEFAULT_LAYOUT.collapsible,
    side: SIDEBAR_SIDES.includes(c.side as SidebarSide) ? (c.side as SidebarSide) : DEFAULT_LAYOUT.side,
  };
}

/** Validate an unknown value (a stored pref, an imported blob) into a well-formed `ThemePreference`.
 *  Unknown mode/radius fall back to the default; an unknown `preset` string is KEPT (it may be a valid
 *  library preset id we resolve at apply-time) but a non-string preset falls back. `custom`/`imported`
 *  are kept only if they are structurally valid `CustomTheme`s — a malformed one is dropped, never
 *  partially applied (fail-closed). A non-object input is DEFAULT_THEME. */
export function normalizeThemePreference(value: unknown): ThemePreference {
  if (!value || typeof value !== "object") return DEFAULT_THEME;

  const c = value as Partial<ThemePreference>;
  const out: ThemePreference = {
    mode: isThemeMode(c.mode) ? c.mode : DEFAULT_THEME.mode,
    preset: typeof c.preset === "string" && c.preset.length > 0 ? c.preset : DEFAULT_THEME.preset,
    radius: isThemeRadius(c.radius) ? c.radius : DEFAULT_THEME.radius,
    layout: normalizeLayout(c.layout),
    // `look` is kept if it names a known pack; an unknown id falls to `default` (fail-open to data,
    // closed to garbage). A missing look = the default (a v1 stored theme has no `look` field).
    look: typeof c.look === "string" && lookById(c.look) ? c.look : DEFAULT_THEME.look,
  };
  // Optional per-axis overrides — each kept only if valid, else dropped (falls back to the look default
  // at resolve time). Malformed new fields fall back PER-AXIS, never the whole blob.
  if (typeof c.fontSans === "string" && fontById(c.fontSans)) out.fontSans = c.fontSans;
  if (typeof c.fontMono === "string" && fontById(c.fontMono)) out.fontMono = c.fontMono;
  if (isSurface(c.surface)) out.surface = c.surface;
  if (isMotion(c.motion)) out.motion = c.motion;
  if (isGlass(c.glass)) out.glass = c.glass;
  // A stored custom/imported theme is validated on the REQUIRED seven, then upgraded by DERIVING any
  // missing widened tones (panel2/overlay/accent2) — so a v1 theme survives migration instead of being
  // dropped. A theme missing a required token is malformed and dropped whole (fail-closed).
  if (isCustomTheme(c.custom)) out.custom = completeCustomTheme(c.custom);
  if (isCustomTheme(c.imported)) out.imported = completeCustomTheme(c.imported);
  return out;
}
