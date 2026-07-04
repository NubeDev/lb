// Parse a pasted tweakcn/shadcn CSS theme block into a base-token `CustomTheme`. A tweakcn export is a
// CSS blob with `:root { --background: …; --primary: …; … }` and `.dark { … }` declaring the shadcn
// vocabulary. We extract each block's custom properties, feed them through the SAME adapter the preset
// library uses (so import and preset share one token bridge), and return `{ light, dark }` base tokens.
// Malformed input FAILS CLOSED — returns null so the caller keeps the current theme, never a partial
// apply (theme-customizer-scope: "malformed input fails closed to the current theme").
//
// One responsibility: pasted-CSS → CustomTheme | null.

import { adaptPreset } from "./preset-adapter";
import type { PresetStyles } from "./theme-preset";
import type { CustomTheme } from "./theme-tokens";

/** Pull the innermost `{ … }` body of the first rule whose selector matches `selector` (`:root` or
 *  `.dark`). Returns "" if absent (a theme with only `:root` is valid — dark falls back to light). */
function blockBody(css: string, selector: string): string {
  // Match `<selector> ... { <body> }` — allow the selector to be part of a group (e.g. `:root, .light`).
  const re = new RegExp(`${selector.replace(".", "\\.")}[^{}]*\\{([^}]*)\\}`, "i");
  const m = css.match(re);
  return m ? m[1] : "";
}

/** Parse `--name: value;` declarations from a block body into a shadcn-vocab style map (drops the `--`). */
function declsToStyles(body: string): PresetStyles {
  const styles: PresetStyles = {};
  for (const decl of body.split(";")) {
    const idx = decl.indexOf(":");
    if (idx < 0) continue;
    const name = decl.slice(0, idx).trim();
    const value = decl.slice(idx + 1).trim();
    if (!name.startsWith("--") || !value) continue;
    styles[name.slice(2)] = value;
  }
  return styles;
}

/** Parse a pasted CSS theme into base-token light/dark palettes, or null if it doesn't yield a usable
 *  theme. Reuses `adaptPreset` so tweakcn's `hsl(...)`/`oklch(...)`/`#hex` values all normalize the same
 *  way preset-library colors do. If `.dark` is absent, dark reuses `:root`'s declarations. */
export function parseImportedTheme(css: string): CustomTheme | null {
  if (typeof css !== "string" || css.trim().length === 0) return null;

  const light = declsToStyles(blockBody(css, ":root"));
  const darkBody = blockBody(css, ".dark");
  const dark = darkBody ? declsToStyles(darkBody) : light;

  // Nothing parsed at all → not a theme.
  if (Object.keys(light).length === 0) return null;

  return adaptPreset({ label: "Imported", styles: { light, dark } });
}
