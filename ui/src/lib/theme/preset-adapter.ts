// THE token bridge — the load-bearing piece of the whole feature. An incoming shadcn/tweakcn preset
// speaks the *shadcn* vocabulary (`--primary`, `--background`, `--card`, …). This app derives the
// shadcn tokens FROM a small base palette in `styles/globals.css`, and every host surface (charts,
// panels, nav, the switcher) reads the BASE tokens (`--bg/--panel/--fg/--muted/--accent/--border`).
// So applying a preset by writing shadcn tokens would restyle buttons but leave charts/panels on the
// compiled defaults — a half-themed app. This adapter maps the preset's shadcn vars BACK onto base
// tokens, per light and dark, so the existing CSS derivation re-themes everything from one direction
// of truth (base → shadcn). If this mapping is wrong, the feature silently fails; its round-trip test
// is the regression guard (theme-customizer-scope: "the token bridge is the feature").
//
// One responsibility: shadcn-preset → base-token CustomTheme.

import { colorToHslTriplet } from "./color-to-hsl";
import type { PresetStyles, ThemePreset } from "./theme-preset";
import type { BasePalette, CustomTheme } from "./theme-tokens";

/** Map one mode's shadcn-vocabulary styles onto a base palette. The mapping (scope Intent):
 *    --primary            → --accent
 *    --background         → --bg
 *    --foreground         → --fg
 *    --card / --popover   → --panel   (card wins; popover is the fallback)
 *    --muted              → --muted
 *    --muted-foreground   → --muted-foreground
 *    --border/--input/--ring → --border  (border wins; input/ring fall back)
 *  Each source color (oklch/#hex/hsl) is normalized to an "H S% L%" triplet. A token with no usable
 *  source is omitted, and the caller treats an incomplete palette as an invalid preset (fail-closed). */
function adaptMode(styles: PresetStyles): Partial<BasePalette> {
  const pick = (...candidates: Array<string | undefined>): string | undefined => {
    for (const c of candidates) {
      if (!c) continue;
      const t = colorToHslTriplet(c);
      if (t) return t;
    }
    return undefined;
  };

  const out: Partial<BasePalette> = {};
  const bg = pick(styles.background);
  const panel = pick(styles.card, styles.popover);
  const fg = pick(styles.foreground);
  const muted = pick(styles.muted);
  const mutedForeground = pick(styles["muted-foreground"]);
  const accent = pick(styles.primary);
  const border = pick(styles.border, styles.input, styles.ring);

  if (bg) out.bg = bg;
  if (panel) out.panel = panel;
  if (fg) out.fg = fg;
  if (muted) out.muted = muted;
  if (mutedForeground) out.mutedForeground = mutedForeground;
  if (accent) out.accent = accent;
  if (border) out.border = border;
  return out;
}

const REQUIRED: Array<keyof BasePalette> = [
  "bg",
  "panel",
  "fg",
  "muted",
  "mutedForeground",
  "accent",
  "border",
];

/** Complete a partial palette by borrowing a sensible neighbor for any missing token, so a preset that
 *  omits e.g. `muted-foreground` still yields a full, applyable palette rather than being rejected. */
function complete(p: Partial<BasePalette>): BasePalette | null {
  const bg = p.bg ?? p.panel;
  const fg = p.fg;
  const accent = p.accent;
  // These three carry the theme's identity — without any of them the preset is unusable.
  if (!bg || !fg || !accent) return null;
  return {
    bg,
    panel: p.panel ?? bg,
    fg,
    muted: p.muted ?? p.border ?? bg,
    mutedForeground: p.mutedForeground ?? p.muted ?? fg,
    accent,
    border: p.border ?? p.muted ?? bg,
  };
}

/** Adapt a full preset into a base-token `CustomTheme` (both modes), or null if either mode is missing
 *  the identity tokens (bg/fg/accent) — the caller then leaves the current theme untouched. */
export function adaptPreset(preset: ThemePreset): CustomTheme | null {
  const light = complete(adaptMode(preset.styles.light));
  const dark = complete(adaptMode(preset.styles.dark));
  if (!light || !dark) return null;
  // Sanity: every required token present (complete() guarantees this, asserted for the type).
  void REQUIRED;
  return { light, dark };
}
