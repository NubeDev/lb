// AA contrast vet for every shipped LOOK in both modes (theme-appearance scope, mandatory). Each look's
// palette (resolved from its preset via the adapter, or the built-in amber static block) must clear AA:
//   - foreground on background  → AA normal (4.5)
//   - foreground on panel       → AA normal (4.5)
//   - accent on background      → AA large/UI (3.0)   [accent is used for chips/links/icons, not body]
// If a shipped look drops below AA, its preset values are wrong — a blocker, per the scope.

import { describe, expect, it } from "vitest";

import { AA_LARGE, AA_NORMAL, contrastRatio } from "./contrast";
import { adaptPreset } from "./preset-adapter";
import { THEME_LOOKS } from "./theme-looks.data";
import { THEME_PRESETS } from "./theme-presets.data";
import type { BasePalette } from "./theme-tokens";

/** The built-in accents' static palettes from globals.css (:root / .dark / [data-theme-accent]) —
 *  the `default` look uses the teal built-in (not a library preset, so the adapter can't produce it).
 *  Kept in lockstep with globals.css by this test's presence — a change there that breaks AA fails
 *  here. All three built-ins share the base neutrals; only the accent swaps. */
const BUILTIN_ACCENTS: Record<string, { light: string; dark: string }> = {
  amber: { light: "32 92% 34%", dark: "34 96% 58%" },
  teal: { light: "178 72% 27%", dark: "174 62% 50%" },
  blue: { light: "216 72% 40%", dark: "216 82% 66%" },
};

function builtinPalette(presetId: string): { light: BasePalette; dark: BasePalette } {
  const accent = BUILTIN_ACCENTS[presetId];
  return {
    light: {
      bg: "210 20% 98.5%", panel: "0 0% 100%", fg: "222 30% 16%", muted: "220 10% 40%",
      mutedForeground: "220 10% 40%", accent: accent.light, border: "215 16% 86%",
      panel2: "210 20% 95%", overlay: "220 25% 8%", accent2: "218 55% 34%",
    },
    dark: {
      bg: "218 22% 7%", panel: "218 18% 10%", fg: "215 20% 92%", muted: "215 10% 66%",
      mutedForeground: "215 10% 66%", accent: accent.dark, border: "217 14% 21%",
      panel2: "217 17% 13%", overlay: "220 30% 3%", accent2: "214 78% 68%",
    },
  };
}

/** Resolve a look's preset to a per-mode base palette. */
function paletteFor(presetId: string): { light: BasePalette; dark: BasePalette } | null {
  if (BUILTIN_ACCENTS[presetId]) return builtinPalette(presetId);
  const entry = THEME_PRESETS.find((p) => p.value === presetId);
  return entry ? adaptPreset(entry.preset) : null;
}

describe("built-in accents pass AA contrast in both modes", () => {
  for (const presetId of Object.keys(BUILTIN_ACCENTS)) {
    it(presetId, () => {
      const pal = builtinPalette(presetId);
      for (const mode of ["light", "dark"] as const) {
        const p = pal[mode];
        expect(contrastRatio(p.fg, p.bg), `${presetId}/${mode} fg-on-bg`).toBeGreaterThanOrEqual(AA_NORMAL);
        expect(contrastRatio(p.fg, p.panel), `${presetId}/${mode} fg-on-panel`).toBeGreaterThanOrEqual(AA_NORMAL);
        expect(contrastRatio(p.accent, p.bg), `${presetId}/${mode} accent-on-bg`).toBeGreaterThanOrEqual(AA_LARGE);
      }
    });
  }
});

describe("shipped looks pass AA contrast in both modes", () => {
  for (const look of THEME_LOOKS) {
    const presetId = look.defaults.preset!;
    it(`${look.id} (${presetId})`, () => {
      const pal = paletteFor(presetId);
      expect(pal, `no palette for look ${look.id} preset ${presetId}`).not.toBeNull();
      for (const mode of ["light", "dark"] as const) {
        const p = pal![mode];
        expect(contrastRatio(p.fg, p.bg), `${look.id}/${mode} fg-on-bg`).toBeGreaterThanOrEqual(AA_NORMAL);
        expect(contrastRatio(p.fg, p.panel), `${look.id}/${mode} fg-on-panel`).toBeGreaterThanOrEqual(AA_NORMAL);
        expect(contrastRatio(p.accent, p.bg), `${look.id}/${mode} accent-on-bg`).toBeGreaterThanOrEqual(AA_LARGE);
      }
    });
  }
});
