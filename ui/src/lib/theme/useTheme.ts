import { useContext } from "react";

import { ThemeContext, type ThemeContextValue } from "./theme-context";

export function useTheme() {
  const context = useContext(ThemeContext);
  if (!context) {
    throw new Error("useTheme must be used within ThemeProvider.");
  }

  return context;
}

/** The non-throwing variant: returns the theme context, or `null` outside a `ThemeProvider`. For a
 *  BROADER consumer that legitimately renders outside the provider (a standalone-mounted ext widget —
 *  same shape as `useDashboardWsOptional`), so `useThemeTokens` can fall back to defaults rather than
 *  crash the tile. The strict `useTheme` above is unchanged for the shell's own surfaces. */
export function useThemeOptional(): ThemeContextValue | null {
  return useContext(ThemeContext);
}
