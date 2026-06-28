import type { ThemePreference } from "./theme-options";

export function applyThemePreference(doc: Document, preference: ThemePreference) {
  const root = doc.documentElement;
  root.classList.toggle("dark", preference.mode === "dark");
  root.dataset.themeAccent = preference.accent;
  root.style.colorScheme = preference.mode;
}
