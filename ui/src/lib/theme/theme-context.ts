import { createContext } from "react";

import type { ThemeLayout, ThemeMode, ThemePreference, ThemeRadius } from "./theme-options";
import type { GlassLevel, Motion, Surface } from "./appearance-axes";
import type { CustomTheme } from "./theme-tokens";

export interface ThemeContextValue {
  theme: ThemePreference;
  /** True once the authoritative prefs value has been reconciled over the first-paint cache. */
  hydrated: boolean;
  setMode: (mode: ThemeMode) => void;
  setPreset: (preset: string) => void;
  setRadius: (radius: ThemeRadius) => void;
  /** Pick a look pack — resets the axes the look defines (per-axis overrides cleared). */
  setLook: (look: string) => void;
  /** Override the sans/mono font family (undefined = inherit the look). */
  setFontSans: (id: string | undefined) => void;
  setFontMono: (id: string | undefined) => void;
  /** Override the surface treatment (undefined = inherit the look). */
  setSurface: (surface: Surface | undefined) => void;
  /** Override the motion profile (undefined = inherit the look). */
  setMotion: (motion: Motion | undefined) => void;
  /** Override the glass intensity (undefined = inherit the look; only bites under a glass surface). */
  setGlass: (glass: GlassLevel | undefined) => void;
  /** Patch the sidebar layout (variant/collapsible/side). */
  setLayout: (patch: Partial<ThemeLayout>) => void;
  /** Apply a hand-tweaked/library custom theme (both modes) — clears `preset`/`imported` precedence. */
  setCustom: (custom: CustomTheme | undefined) => void;
  /** Apply a pasted/imported theme (both modes). */
  setImported: (imported: CustomTheme | undefined) => void;
  /** Replace the whole preference (the Customizer's bulk setter). */
  setTheme: (theme: ThemePreference) => void;
  /** Reset to the built-in default and persist the reset. */
  reset: () => void;
}

export const ThemeContext = createContext<ThemeContextValue | null>(null);
