// Apply a `ThemePreference` to the document root — the ONE place the theme touches the DOM. It sets:
//   - the `.dark` class + `color-scheme` (mode),
//   - `--radius` (a length),
//   - and, for a custom/imported/library-preset theme, the BASE tokens inline on `<html>` so
//     `globals.css` derives the shadcn tokens and every host surface re-themes. For a built-in accent
//     preset it instead sets `data-theme-accent` and CLEARS any inline base tokens, letting the static
//     `:root`/`.dark` blocks own the values.
// Writing base tokens (not shadcn tokens) is the load-bearing choice — see `preset-adapter.ts`.
//
// One responsibility: preference → DOM writes. Palette resolution lives in `theme-resolve.ts`.

import { BASE_TOKENS } from "./theme-tokens";
import { resolveAccentAttr, resolvePalette } from "./theme-resolve";
import type { ThemePreference } from "./theme-options";

export function applyThemePreference(doc: Document, pref: ThemePreference) {
  const root = doc.documentElement;
  root.classList.toggle("dark", pref.mode === "dark");
  root.style.colorScheme = pref.mode;
  root.style.setProperty("--radius", pref.radius);

  const palette = resolvePalette(pref);
  if (palette) {
    // Inline base tokens win over the static blocks; a built-in accent attribute would fight them.
    root.removeAttribute("data-theme-accent");
    for (const { key, cssVar } of BASE_TOKENS) {
      // A derived tone may be absent on a not-yet-normalized palette; skip it so the static block's
      // value (or a previously-written one) stays rather than clobbering the token with "".
      const value = palette[key];
      if (value) root.style.setProperty(cssVar, value);
      else root.style.removeProperty(cssVar);
    }
  } else {
    // Built-in accent path: clear any inline base tokens from a previous custom theme, set the attr.
    for (const { cssVar } of BASE_TOKENS) {
      root.style.removeProperty(cssVar);
    }
    const accent = resolveAccentAttr(pref);
    if (accent) root.dataset.themeAccent = accent;
    else root.removeAttribute("data-theme-accent");
  }
}
