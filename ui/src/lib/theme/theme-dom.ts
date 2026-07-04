// Apply a `ThemePreference` to the document root — the ONE place the theme touches the DOM. It resolves
// the appearance (look fold) once, then sets:
//   - the `.dark` class + `color-scheme` (mode),
//   - `--radius` (a length, from the resolved axis),
//   - the resolved BASE tokens inline for a custom/imported/library-preset theme (or `data-theme-accent`
//     + cleared inline tokens for a built-in accent), driven by the RESOLVED preset (a look can change it),
//   - `--font-sans` / `--font-mono` (the resolved font stacks),
//   - `data-surface` (flat|elevated|glass) and `data-motion` (off|subtle|full) attributes, which
//     `globals.css` + `lib/motion` read so panels/chrome pick up the treatment by cascade.
// Writing base tokens (not shadcn tokens) is the load-bearing choice — see `preset-adapter.ts`.
//
// One responsibility: preference → DOM writes. Palette/appearance resolution live in their own files.

import { BASE_TOKENS } from "./theme-tokens";
import { resolveAccentAttr, resolvePalette } from "./theme-resolve";
import { resolveAppearance } from "./look-resolve";
import { resolveMotion } from "./resolve-motion";
import { fontById } from "./theme-fonts.data";
import type { ThemePreference } from "./theme-options";

export function applyThemePreference(doc: Document, pref: ThemePreference) {
  const root = doc.documentElement;
  const appearance = resolveAppearance(pref);

  root.classList.toggle("dark", pref.mode === "dark");
  root.style.colorScheme = pref.mode;
  root.style.setProperty("--radius", appearance.radius);

  // A look can change which preset drives the palette; fold it in before resolving colors.
  const effective: ThemePreference = { ...pref, preset: appearance.preset };
  const palette = resolvePalette(effective);
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
    const accent = resolveAccentAttr(effective);
    if (accent) root.dataset.themeAccent = accent;
    else root.removeAttribute("data-theme-accent");
  }

  // Font tokens — the resolved family's stack (system stacks need no download; see `font-loader.ts`).
  root.style.setProperty("--font-sans", fontById(appearance.fontSans)?.stack ?? "");
  root.style.setProperty("--font-mono", fontById(appearance.fontMono)?.stack ?? "");

  // Surface + motion attributes — CSS + lib/motion read these. Motion honors prefers-reduced-motion.
  root.dataset.surface = appearance.surface;
  root.dataset.motion = resolveMotion(appearance.motion, doc);
}
