import { DEFAULT_THEME, THEME_STORAGE_KEY, normalizeThemePreference, type ThemePreference } from "./theme-options";

interface ThemeStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

function parseTheme(raw: string | null): ThemePreference {
  if (!raw) return DEFAULT_THEME;

  try {
    return normalizeThemePreference(JSON.parse(raw));
  } catch {
    return DEFAULT_THEME;
  }
}

export function loadThemePreference(storage: ThemeStorage | undefined = globalThis.localStorage): ThemePreference {
  if (!storage) return DEFAULT_THEME;

  try {
    return parseTheme(storage.getItem(THEME_STORAGE_KEY));
  } catch {
    return DEFAULT_THEME;
  }
}

export function saveThemePreference(
  preference: ThemePreference,
  storage: ThemeStorage | undefined = globalThis.localStorage,
) {
  if (!storage) return;

  try {
    storage.setItem(THEME_STORAGE_KEY, JSON.stringify(preference));
  } catch {
    // Local storage can be unavailable in private modes or locked-down webviews.
  }
}
