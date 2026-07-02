// The design tokens — the look's single source of truth (look-scope.md §visual-language).
// Nothing outside theme/ hardcodes a color. Surface/accent/text colors are BOUND TO THE HOST SHELL
// tokens (theme/host-tokens.ts reads the shell's CSS vars at runtime), so the canvas follows the
// host light/dark toggle and accent swatch — exactly what "the framework binds these to shell
// tokens" always meant. `refreshTokens()` re-reads them on a theme change (SceneCanvas wires it to
// subscribeThemeChange). Medium + status hues stay fixed: they encode meaning (chw vs hw, running
// vs fault), not chrome, so they must not drift with the theme.

import { readCanvasColors, type CanvasColors } from "./host-tokens";

/** Live host-derived colors — reassigned in place by refreshTokens() so every reader sees the swap
 *  without re-importing. Seeded from the current host theme at module load. */
let host: CanvasColors = readCanvasColors();

/** Re-read the host tokens after a theme change. Returns the fresh colors (SceneCanvas also uses
 *  the return to drive the r3f <color>/light props, which are React-owned, not material-cached). */
export function refreshTokens(): CanvasColors {
  host = readCanvasColors();
  tokens.color.canvas = host.canvas;
  tokens.color.steel = host.steel;
  tokens.color.grid = host.grid;
  tokens.color.accent = host.accent;
  tokens.color.duct = host.duct;
  tokens.color.medium.air = host.accent; // supply/return air shares the accent family
  tokens.color.text.label = host.textLabel;
  tokens.color.text.value = host.textValue;
  return host;
}

export const tokens = {
  color: {
    canvas: host.canvas, // scene background + ground (host --bg)
    steel: host.steel, // equipment bodies (desaturated), from the surface ramp
    grid: host.grid, // ground grid, fades with zoom
    accent: host.accent, // live data + selection ONLY (host --accent)
    /** per-medium accents (look-scope: per-medium, capped at 3). `air` follows the accent; chw/hw
     *  are FIXED semantic hues (blue = chilled, orange = hot) — meaning, not chrome. */
    medium: {
      air: host.accent, // supply/return air — shares the accent family
      chw: "#60a5fa", // chilled water (fixed)
      hw: "#fb923c", // hot water (fixed)
    },
    duct: host.duct, // duct body — slightly lighter than canvas
    text: {
      label: host.textLabel, // secondary labels (host --muted)
      value: host.textValue, // live values (host --fg)
    },
    /** status hues are FIXED semantic colors — running/stopped/fault/override read the same in any
     *  theme, so they never derive from the accent swatch. */
    status: {
      running: "#2dd4bf",
      stopped: "#4b5563",
      fault: "#f59e0b", // pulses toward #ef4444
      override: "#a78bfa",
    },
  },
  motion: {
    feedbackMs: 150,
    cameraSpringMs: 600,
    faultPulseHz: 0.5,
  },
  grid: {
    step: 8, // snap grid, world units
  },
};
