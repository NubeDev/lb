// The theme ⇄ prefs bridge — persist and read the member's `ThemePreference` through the shipped
// `prefs` verbs, riding the `ui_theme` axis on the prefs record (no new verb/table/cap; see the
// theme-customizer scope's persistence correction). Three operations, mapping 1:1 to prefs verbs:
//   - readResolvedTheme() → `prefs.resolve` (member → workspace-default → built-in fold; opaque blob
//     normalized to a ThemePreference here, fail-closed to DEFAULT_THEME).
//   - persistTheme(pref)  → `prefs.set` on the caller's OWN record (member preference).
//   - persistWorkspaceDefaultTheme(pref) → `prefs.set_default` (admin-gated workspace default).
// The theme is prefs' authority; `theme-storage.ts` (localStorage) is only the first-paint cache.
//
// One responsibility: theme persistence over prefs.

import { getPrefs } from "@/lib/prefs/get";
import { resolvePrefs } from "@/lib/prefs/resolve";
import { setDefaultPrefs, setPrefs } from "@/lib/prefs/set";
import { DEFAULT_THEME, normalizeThemePreference, type ThemePreference } from "./theme-options";

/** Resolve the viewer's theme from the prefs chain, normalized. Returns null when NO theme is set
 *  anywhere (so the caller keeps the local cache / default rather than clobbering it with a default). */
export async function readResolvedTheme(): Promise<ThemePreference | null> {
  const resolved = await resolvePrefs();
  if (resolved.ui_theme == null) return null;
  return normalizeThemePreference(resolved.ui_theme);
}

/** Read ONLY the viewer's own stored theme (not the folded chain) — the right shape for the Customizer
 *  to know whether the member has an explicit theme vs inheriting the workspace default. Null if unset. */
export async function readOwnTheme(): Promise<ThemePreference | null> {
  const own = await getPrefs();
  if (!own || own.ui_theme == null) return null;
  return normalizeThemePreference(own.ui_theme);
}

/** Persist `pref` as the viewer's OWN theme (member-level). Requires `mcp:prefs.set:call`; a member
 *  without it is denied (the caller degrades to local-only). */
export function persistTheme(pref: ThemePreference): Promise<void> {
  return setPrefs({ ui_theme: pref });
}

/** Persist `pref` as the WORKSPACE-DEFAULT theme (admin-gated `mcp:prefs.set_default:call`). */
export function persistWorkspaceDefaultTheme(pref: ThemePreference): Promise<void> {
  return setDefaultPrefs({ ui_theme: pref });
}

/** Reset the viewer's own theme to inherit the chain again (clear the axis). Writing DEFAULT_THEME as
 *  an explicit member value would shadow the workspace default; to truly reset, we set the built-in
 *  default explicitly (member choice = the default) — a true "unset" would need a null-capable patch,
 *  which the prefs merge does not express, so an explicit default is the honest reset. */
export function resetTheme(): Promise<void> {
  return setPrefs({ ui_theme: DEFAULT_THEME });
}
