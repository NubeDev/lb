import { createContext } from "react";

import type { ThemeAccent, ThemeMode, ThemePreference } from "./theme-options";

export interface ThemeContextValue {
  theme: ThemePreference;
  setMode: (mode: ThemeMode) => void;
  setAccent: (accent: ThemeAccent) => void;
  setTheme: (theme: ThemePreference) => void;
}

export const ThemeContext = createContext<ThemeContextValue | null>(null);
