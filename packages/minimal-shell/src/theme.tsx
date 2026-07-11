// Minimal theme provider — applies CSS variables from prefs/localStorage to documentElement.
// Extensions inherit via the cascade (no :root block in the ext — the scoped root inherits).
import { useEffect, useState } from "react";
import type { ReactNode } from "react";

const KEY = "lb.theme";

type Theme = "light" | "dark";

function loadTheme(): Theme {
  try {
    const t = JSON.parse(localStorage.getItem(KEY) || "null");
    if (t?.mode === "dark") return "dark";
    if (t?.mode === "light") return "light";
  } catch {}
  // No persisted choice → follow the system (the ThemeProvider re-checks on OS changes).
  if (typeof matchMedia !== "undefined" && matchMedia("(prefers-color-scheme: dark)").matches) {
    return "dark";
  }
  return "light";
}

function applyTheme(theme: Theme) {
  document.documentElement.style.colorScheme = theme;
  document.documentElement.dataset.themeMode = theme;
  // The shadcn-compatible token swap: extension UIs style off a host-level `.dark` class, so a
  // host toggle propagates into every mounted extension via the CSS cascade.
  document.documentElement.classList.toggle("dark", theme === "dark");
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setTheme] = useState<Theme>(loadTheme);

  useEffect(() => {
    applyTheme(theme);
    localStorage.setItem(KEY, JSON.stringify({ mode: theme }));
  }, [theme]);

  return (
    <ThemeContext.Provider value={{ theme, setTheme }}>
      {children}
    </ThemeContext.Provider>
  );
}

import { createContext, useContext } from "react";
interface ThemeCtx {
  theme: Theme;
  setTheme: (t: Theme) => void;
}
const ThemeContext = createContext<ThemeCtx>({ theme: "light", setTheme: () => {} });
export function useTheme() {
  return useContext(ThemeContext);
}
