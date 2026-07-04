// The base-token contract — the palette the Customizer writes, and that EVERY host surface reads
// (charts, panels, nav; `globals.css` derives the shadcn tokens from these). A token value is an HSL
// channel triplet string like "40 30% 96%", consumed as `hsl(var(--…))`. This file owns the token
// identity list + the palette types; the ADAPTER (`preset-adapter.ts`) produces these from a preset,
// the DOM layer (`theme-dom.ts`) writes them, the color-picker edits them, `derive-tones.ts` fills the
// widened tones from the required seven.
//
// TONE WIDENING (theme-appearance scope): the palette is split into REQUIRED tokens (the shipped seven —
// every stored theme has them; `isBasePalette` still demands them) and DERIVED tones (`panel2`,
// `overlay`, `accent2` — new, and *derivable* from the seven). Validation therefore does NOT require the
// derived tones: a v1 stored theme normalizes by DERIVING them, never fails closed and drops the custom
// palette (the migration blocker the scope names). Semantic `--success`/`--warning` are fixed tokens in
// `globals.css` (like `--destructive`), not member-editable here — so they are not palette keys.
//
// One responsibility: the token contract + palette types.

/** The seven REQUIRED base color tokens (radius is a separate length, handled in `theme-dom`). Every
 *  stored/imported theme must carry all seven; `isBasePalette` enforces it. */
export const REQUIRED_TOKENS = [
  { key: "bg", cssVar: "--bg", label: "Background" },
  { key: "panel", cssVar: "--panel", label: "Panel" },
  { key: "fg", cssVar: "--fg", label: "Foreground" },
  { key: "muted", cssVar: "--muted", label: "Muted" },
  { key: "mutedForeground", cssVar: "--muted-foreground", label: "Muted foreground" },
  { key: "accent", cssVar: "--accent", label: "Accent" },
  { key: "border", cssVar: "--border", label: "Border" },
] as const;

/** The DERIVED tones — new with the tone widening, *derivable* from the required seven, so a stored v1
 *  theme (which lacks them) is upgraded by derivation rather than rejected. Editable in Brand Colors. */
export const DERIVED_TOKENS = [
  { key: "panel2", cssVar: "--panel-2", label: "Raised panel" },
  { key: "overlay", cssVar: "--overlay", label: "Overlay" },
  { key: "accent2", cssVar: "--accent-2", label: "Secondary accent" },
] as const;

/** All writable base tokens = required + derived. `BASE_TOKENS` stays the name the DOM writer + Brand
 *  Colors iterate, now covering the widened set. */
export const BASE_TOKENS = [...REQUIRED_TOKENS, ...DERIVED_TOKENS] as const;

export type RequiredTokenKey = (typeof REQUIRED_TOKENS)[number]["key"];
export type DerivedTokenKey = (typeof DERIVED_TOKENS)[number]["key"];
export type BaseTokenKey = RequiredTokenKey | DerivedTokenKey;

/** The required-seven slice of a palette (an HSL triplet per key). */
export type RequiredPalette = Record<RequiredTokenKey, string>;
/** The derived-tones slice of a palette. */
export type DerivedTones = Record<DerivedTokenKey, string>;
/** One mode's full resolved base palette — required + derived, every value an "H S% L%" triplet. */
export type BasePalette = RequiredPalette & DerivedTones;

/** A full custom/imported theme: the base palette for each mode. Light/dark are kept distinct so a
 *  mode flip re-applies the correct variant. */
export interface CustomTheme {
  light: BasePalette;
  dark: BasePalette;
}

const REQUIRED_KEYS: readonly RequiredTokenKey[] = REQUIRED_TOKENS.map((t) => t.key);

/** A value is a valid base palette iff every REQUIRED token key is present as a non-empty string. The
 *  derived tones are NOT required here (a v1 theme lacks them) — they are filled by `completeTones`. */
export function isBasePalette(value: unknown): value is RequiredPalette {
  if (!value || typeof value !== "object") return false;
  const v = value as Record<string, unknown>;
  return REQUIRED_KEYS.every((k) => typeof v[k] === "string" && (v[k] as string).length > 0);
}

/** A value is a valid CustomTheme iff both modes carry the required seven. Derived tones optional; a
 *  malformed stored/imported theme (missing a required token) is dropped, never partially applied. */
export function isCustomTheme(value: unknown): value is CustomTheme {
  if (!value || typeof value !== "object") return false;
  const v = value as Record<string, unknown>;
  return isBasePalette(v.light) && isBasePalette(v.dark);
}
