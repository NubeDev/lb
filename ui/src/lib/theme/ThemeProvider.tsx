import { useCallback, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";

import { applyThemePreference } from "./theme-dom";
import { ThemeContext } from "./theme-context";
import { loadThemePreference, saveThemePreference } from "./theme-storage";
import { useThemePersist } from "./useThemePersist";
import {
  DEFAULT_THEME,
  type ThemeLayout,
  type ThemeMode,
  type ThemePreference,
  type ThemeRadius,
} from "./theme-options";
import type { CustomTheme } from "./theme-tokens";

interface Props {
  children: ReactNode;
}

export function ThemeProvider({ children }: Props) {
  // First paint: the localStorage cache (no flash). Prefs is the authority, reconciled on mount.
  const [theme, setThemeState] = useState<ThemePreference>(() => loadThemePreference());
  const [hydrated, setHydrated] = useState(false);
  // A user edit this session — guards the mount reconcile from clobbering a fresh local pick, and gates
  // debounced persistence (we only write what the user changed, not the reconciled-from-prefs value).
  const [dirty, setDirty] = useState(false);
  const dirtyOnce = useRef(false);

  useLayoutEffect(() => {
    applyThemePreference(document, theme);
    saveThemePreference(theme);
  }, [theme]);

  // A local edit: mark dirty and update state+cache. Persistence + reconcile are handled by the hook.
  const edit = useCallback((next: ThemePreference | ((cur: ThemePreference) => ThemePreference)) => {
    dirtyOnce.current = true;
    setDirty(true);
    setThemeState(next);
  }, []);

  useThemePersist({
    theme,
    dirty,
    onReconciled: (fromPrefs) => {
      if (!dirtyOnce.current) setThemeState(fromPrefs);
    },
    onHydrated: () => setHydrated(true),
  });

  const setMode = useCallback((mode: ThemeMode) => edit((c) => ({ ...c, mode })), [edit]);
  const setPreset = useCallback(
    (preset: string) => edit((c) => ({ ...c, preset, custom: undefined, imported: undefined })),
    [edit],
  );
  const setRadius = useCallback((radius: ThemeRadius) => edit((c) => ({ ...c, radius })), [edit]);
  const setLayout = useCallback(
    (patch: Partial<ThemeLayout>) => edit((c) => ({ ...c, layout: { ...c.layout, ...patch } })),
    [edit],
  );
  const setCustom = useCallback(
    (custom: CustomTheme | undefined) => edit((c) => ({ ...c, custom, imported: undefined })),
    [edit],
  );
  const setImported = useCallback(
    (imported: CustomTheme | undefined) => edit((c) => ({ ...c, imported, custom: undefined })),
    [edit],
  );
  const setTheme = useCallback((next: ThemePreference) => edit(next), [edit]);
  const reset = useCallback(() => edit(DEFAULT_THEME), [edit]);

  const value = useMemo(
    () => ({ theme, hydrated, setMode, setPreset, setRadius, setLayout, setCustom, setImported, setTheme, reset }),
    [theme, hydrated, setMode, setPreset, setRadius, setLayout, setCustom, setImported, setTheme, reset],
  );

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}
