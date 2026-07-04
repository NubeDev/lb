// The base-token contract — the palette the Customizer actually writes, and that EVERY host surface
// reads (charts, panels, nav, `globals.css` derives the shadcn tokens from these). A token value is an
// HSL channel triplet string like "40 30% 96%", consumed as `hsl(var(--bg))` — the project convention
// in `styles/globals.css` and `features/charts/chartTheme.ts`. This file owns the token identity list
// and the palette types; the ADAPTER (`preset-adapter.ts`) produces these from a preset, the DOM layer
// (`theme-dom.ts`) writes them, and the color-picker edits them. One responsibility: the token contract.

/** The seven writable base color tokens (radius is a separate length, handled in `theme-dom`). The key
 *  is the camelCase field on a `BasePalette`; `cssVar` is the `--…` custom property in `globals.css`. */
export const BASE_TOKENS = [
  { key: "bg", cssVar: "--bg", label: "Background" },
  { key: "panel", cssVar: "--panel", label: "Panel" },
  { key: "fg", cssVar: "--fg", label: "Foreground" },
  { key: "muted", cssVar: "--muted", label: "Muted" },
  { key: "mutedForeground", cssVar: "--muted-foreground", label: "Muted foreground" },
  { key: "accent", cssVar: "--accent", label: "Accent" },
  { key: "border", cssVar: "--border", label: "Border" },
] as const;

export type BaseTokenKey = (typeof BASE_TOKENS)[number]["key"];

/** One mode's worth of resolved base tokens — every value an HSL triplet string ("H S% L%"). */
export type BasePalette = Record<BaseTokenKey, string>;

/** A full custom/imported theme: the base palette for each mode. Light/dark are kept distinct so a
 *  mode flip re-applies the correct variant (the template's `isDarkMode` re-apply behaviour). */
export interface CustomTheme {
  light: BasePalette;
  dark: BasePalette;
}

const TOKEN_KEYS: readonly BaseTokenKey[] = BASE_TOKENS.map((t) => t.key);

/** A value is a valid base palette iff every token key is present as a non-empty string. */
export function isBasePalette(value: unknown): value is BasePalette {
  if (!value || typeof value !== "object") return false;
  const v = value as Record<string, unknown>;
  return TOKEN_KEYS.every((k) => typeof v[k] === "string" && (v[k] as string).length > 0);
}

/** A value is a valid CustomTheme iff both modes are valid base palettes. Used to fail-closed on a
 *  malformed stored/imported theme (drop it entirely, never partially apply). */
export function isCustomTheme(value: unknown): value is CustomTheme {
  if (!value || typeof value !== "object") return false;
  const v = value as Record<string, unknown>;
  return isBasePalette(v.light) && isBasePalette(v.dark);
}
