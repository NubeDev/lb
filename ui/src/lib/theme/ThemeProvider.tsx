import { useCallback, useLayoutEffect, useMemo, useState, type ReactNode } from "react";

import { applyThemePreference } from "./theme-dom";
import { ThemeContext } from "./theme-context";
import { loadThemePreference, saveThemePreference } from "./theme-storage";
import type { ThemeAccent, ThemeMode, ThemePreference } from "./theme-options";

interface Props {
  children: ReactNode;
}

export function ThemeProvider({ children }: Props) {
  const [theme, setTheme] = useState<ThemePreference>(() => loadThemePreference());

  useLayoutEffect(() => {
    applyThemePreference(document, theme);
    saveThemePreference(theme);
  }, [theme]);

  const setMode = useCallback((mode: ThemeMode) => {
    setTheme((current) => ({ ...current, mode }));
  }, []);

  const setAccent = useCallback((accent: ThemeAccent) => {
    setTheme((current) => ({ ...current, accent }));
  }, []);

  const value = useMemo(
    () => ({
      theme,
      setMode,
      setAccent,
      setTheme,
    }),
    [setAccent, setMode, theme],
  );

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}
