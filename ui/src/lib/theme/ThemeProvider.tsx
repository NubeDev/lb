import { useCallback, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";

import { applyThemePreference } from "./theme-dom";
import { resolveAppearance } from "./look-resolve";
import { loadFont } from "./font-loader";
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
import { applyLook } from "./look-resolve";
import type { GlassLevel, Motion, Surface } from "./appearance-axes";
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
    // Lazy-load the resolved font faces (system stacks are a no-op) so a picked family self-hosts. The
    // token stack is already written, so text renders in the fallback until the woff2 arrives (no FOIT).
    const appearance = resolveAppearance(theme);
    loadFont(appearance.fontSans);
    loadFont(appearance.fontMono);
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
  // Picking a look resets the axes it defines (drops per-axis overrides) so it lands like its thumbnail.
  const setLook = useCallback((look: string) => edit((c) => applyLook(c, look)), [edit]);
  const setFontSans = useCallback((fontSans: string | undefined) => edit((c) => ({ ...c, fontSans })), [edit]);
  const setFontMono = useCallback((fontMono: string | undefined) => edit((c) => ({ ...c, fontMono })), [edit]);
  const setSurface = useCallback((surface: Surface | undefined) => edit((c) => ({ ...c, surface })), [edit]);
  const setMotion = useCallback((motion: Motion | undefined) => edit((c) => ({ ...c, motion })), [edit]);
  const setGlass = useCallback((glass: GlassLevel | undefined) => edit((c) => ({ ...c, glass })), [edit]);
  // Set one icon's color, or clear it. Clears to undefined when the last entry is removed so the field
  // fully disappears (turning colorization OFF) — matches "presence === ON".
  const setIconColor = useCallback(
    (surface: string, color: string | undefined) =>
      edit((c) => {
        const next = { ...(c.iconColors ?? {}) };
        if (color === undefined) delete next[surface];
        else next[surface] = color;
        const iconColors = Object.keys(next).length > 0 ? next : undefined;
        return { ...c, iconColors };
      }),
    [edit],
  );
  // Replace the whole icon-color map (auto-assign writes here). undefined/empty = colorization OFF.
  const setIconColors = useCallback(
    (colors: Record<string, string> | undefined) =>
      edit((c) => ({
        ...c,
        iconColors: colors && Object.keys(colors).length > 0 ? colors : undefined,
      })),
    [edit],
  );
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
    () => ({
      theme,
      hydrated,
      setMode,
      setPreset,
      setRadius,
      setLook,
      setFontSans,
      setFontMono,
      setSurface,
      setMotion,
      setGlass,
      setIconColor,
      setIconColors,
      setLayout,
      setCustom,
      setImported,
      setTheme,
      reset,
    }),
    [
      theme,
      hydrated,
      setMode,
      setPreset,
      setRadius,
      setLook,
      setFontSans,
      setFontMono,
      setSurface,
      setMotion,
      setGlass,
      setIconColor,
      setIconColors,
      setLayout,
      setCustom,
      setImported,
      setTheme,
      reset,
    ],
  );

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}
