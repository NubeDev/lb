// The `ctx.theme` resolver — produces the WIDENED shape (base + tones + radius + fonts + surface + motion
// + chart ramp) as concrete strings from the applied DOM + the preference.

import { describe, expect, it } from "vitest";

import { resolveThemeTokens } from "./resolve-theme-tokens";
import { applyThemePreference } from "./theme-dom";
import { DEFAULT_THEME, type ThemePreference } from "./theme-options";
import type { CustomTheme } from "./theme-tokens";

const pref = (p: Partial<ThemePreference>): ThemePreference => ({ ...DEFAULT_THEME, ...p });

describe("resolveThemeTokens", () => {
  it("resolves the widened shape as concrete strings", () => {
    const doc = document.implementation.createHTMLDocument("theme");
    applyThemePreference(doc, pref({ look: "glass" }));
    const t = resolveThemeTokens(pref({ look: "glass" }), doc.documentElement, doc);

    // Every widened key present.
    for (const key of ["bg", "panel", "fg", "muted", "mutedForeground", "accent", "border", "panel2", "overlay", "accent2", "radius", "fontSans", "fontMono", "surface", "motion", "chart"] as const) {
      expect(t).toHaveProperty(key);
    }
    // Non-color axes come from the resolved appearance (glass look → glass surface / full motion / inter).
    expect(t.surface).toBe("glass");
    expect(t.motion).toBe("full");
    expect(t.fontSans).toContain("Inter");
    // The chart ramp is the core categorical ramp (8 hues).
    expect(Array.isArray(t.chart)).toBe(true);
    expect(t.chart.length).toBe(8);
  });

  it("honors an inline custom palette (reads computed, not just the preset)", () => {
    const doc = document.implementation.createHTMLDocument("theme");
    const CUSTOM: CustomTheme = {
      light: { bg: "0 0% 100%", panel: "0 0% 96%", fg: "0 0% 10%", muted: "0 0% 40%", mutedForeground: "0 0% 45%", accent: "300 80% 50%", border: "0 0% 85%", panel2: "0 0% 92%", overlay: "0 0% 20%", accent2: "180 80% 40%" },
      dark: { bg: "0 0% 5%", panel: "0 0% 9%", fg: "0 0% 92%", muted: "0 0% 60%", mutedForeground: "0 0% 62%", accent: "300 80% 60%", border: "0 0% 18%", panel2: "0 0% 13%", overlay: "0 0% 80%", accent2: "180 80% 55%" },
    };
    const p = pref({ mode: "dark", custom: CUSTOM });
    applyThemePreference(doc, p);
    const t = resolveThemeTokens(p, doc.documentElement, doc);
    // The inline accent (custom) is read off the computed root and wrapped as hsl(...).
    expect(t.accent).toBe("hsl(300 80% 60%)");
    expect(t.accent2).toBe("hsl(180 80% 55%)");
  });
});
