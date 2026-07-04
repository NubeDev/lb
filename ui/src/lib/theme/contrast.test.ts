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

/** The built-in amber accent's static palette from globals.css (:root / .dark), which the `default`
 *  look uses (it isn't a library preset, so the adapter can't produce it). Kept in lockstep with
 *  globals.css by this test's presence — a change there that breaks AA fails here. */
const AMBER: { light: BasePalette; dark: BasePalette } = {
  light: {
    bg: "40 30% 96%", panel: "40 24% 92%", fg: "30 12% 14%", muted: "30 6% 44%",
    mutedForeground: "30 6% 44%", accent: "32 92% 34%", border: "40 12% 82%",
    panel2: "40 26% 88%", overlay: "30 10% 20%", accent2: "178 60% 32%",
  },
  dark: {
    bg: "24 8% 8%", panel: "24 7% 11%", fg: "36 16% 90%", muted: "30 5% 56%",
    mutedForeground: "30 5% 56%", accent: "34 96% 58%", border: "24 6% 18%",
    panel2: "24 7% 15%", overlay: "36 12% 82%", accent2: "176 62% 52%",
  },
};

/** Resolve a look's preset to a per-mode base palette. */
function paletteFor(presetId: string): { light: BasePalette; dark: BasePalette } | null {
  if (presetId === "amber") return AMBER;
  const entry = THEME_PRESETS.find((p) => p.value === presetId);
  return entry ? adaptPreset(entry.preset) : null;
}

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
