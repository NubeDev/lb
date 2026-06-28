export const THEME_MODES = ["dark", "light"] as const;
export const THEME_ACCENTS = ["amber", "teal", "blue"] as const;

export type ThemeMode = (typeof THEME_MODES)[number];
export type ThemeAccent = (typeof THEME_ACCENTS)[number];

export interface ThemePreference {
  mode: ThemeMode;
  accent: ThemeAccent;
}

export const DEFAULT_THEME: ThemePreference = {
  mode: "dark",
  accent: "amber",
};

export const THEME_STORAGE_KEY = "lb.theme";

export const THEME_MODE_OPTIONS: ReadonlyArray<{ value: ThemeMode; label: string }> = [
  { value: "dark", label: "Dark" },
  { value: "light", label: "Light" },
];

export const THEME_ACCENT_OPTIONS: ReadonlyArray<{ value: ThemeAccent; label: string }> = [
  { value: "amber", label: "Amber" },
  { value: "teal", label: "Teal" },
  { value: "blue", label: "Blue" },
];

export function isThemeMode(value: unknown): value is ThemeMode {
  return typeof value === "string" && THEME_MODES.includes(value as ThemeMode);
}

export function isThemeAccent(value: unknown): value is ThemeAccent {
  return typeof value === "string" && THEME_ACCENTS.includes(value as ThemeAccent);
}

export function normalizeThemePreference(value: unknown): ThemePreference {
  if (!value || typeof value !== "object") return DEFAULT_THEME;

  const candidate = value as Partial<ThemePreference>;
  return {
    mode: isThemeMode(candidate.mode) ? candidate.mode : DEFAULT_THEME.mode,
    accent: isThemeAccent(candidate.accent) ? candidate.accent : DEFAULT_THEME.accent,
  };
}
