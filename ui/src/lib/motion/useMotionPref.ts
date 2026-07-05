// The motion seam's hook — resolves the EFFECTIVE motion level for the current member, honoring
// `prefers-reduced-motion`, and hands callers the knobs they need to gate the `motion` (motion.dev)
// animations in shell chrome. `off` → no JS motion (return zero-duration / no variants); `subtle` →
// short, small-distance variants; `full` → the designed set. This is the JS counterpart to the
// `data-motion` CSS fence, both reading the same resolved value (`resolve-motion.ts`), so the off switch
// is honored in one place whether the animation is CSS or JS.
//
// One responsibility: current member → effective motion + a duration/enabled helper.

import { useThemeOptional } from "@/lib/theme";
import { DEFAULT_THEME, resolveAppearance, resolveMotion, type Motion } from "@/lib/theme";

export interface MotionPref {
  /** The effective level after the reduced-motion fold. */
  motion: Motion;
  /** True when any JS motion should run (`subtle` or `full`). */
  enabled: boolean;
  /** Scale a base duration to the level: 0 when off, ~0.6× for subtle, 1× for full. */
  duration: (base: number) => number;
  /** Scale a base travel distance (px) the same way — subtle moves less. */
  distance: (base: number) => number;
}

export function useMotionPref(): MotionPref {
  // Optional context (not the throwing `useTheme`) so motion-wrapped surfaces also render outside a
  // ThemeProvider (embedded/test mounts) — they fall back to the default preference, like CodeEditor.
  const theme = useThemeOptional()?.theme ?? DEFAULT_THEME;
  const motion = resolveMotion(resolveAppearance(theme).motion);
  const enabled = motion !== "off";
  const scale = motion === "full" ? 1 : motion === "subtle" ? 0.6 : 0;
  return {
    motion,
    enabled,
    duration: (base) => base * scale,
    distance: (base) => (motion === "subtle" ? base * 0.5 : enabled ? base : 0),
  };
}
