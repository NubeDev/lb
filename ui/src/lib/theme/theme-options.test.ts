// Normalization + the tone-widening MIGRATION FOLD (theme-appearance scope). The load-bearing case:
// a stored v1 customizer preference (no look/font/surface/motion, a SEVEN-token custom palette)
// normalizes to a working theme WITHOUT dropping the custom palette — the derived tones are filled,
// never fail-closed to DEFAULT_THEME. A theme missing a REQUIRED token is still dropped (fail-closed).

import { describe, expect, it } from "vitest";

import { DEFAULT_THEME, DEFAULT_LAYOUT, normalizeThemePreference } from "./theme-options";
import { deriveTones } from "./derive-tones";
import type { RequiredPalette } from "./theme-tokens";

const V1_LIGHT: RequiredPalette = {
  bg: "40 30% 96%",
  panel: "40 24% 92%",
  fg: "30 12% 14%",
  muted: "30 6% 44%",
  mutedForeground: "30 6% 40%",
  accent: "178 70% 28%",
  border: "40 12% 82%",
};
const V1_DARK: RequiredPalette = {
  bg: "24 8% 8%",
  panel: "24 7% 11%",
  fg: "36 16% 90%",
  muted: "30 5% 56%",
  mutedForeground: "30 5% 60%",
  accent: "176 68% 50%",
  border: "24 6% 18%",
};

/** A stored v1 preference: pre-widening shape, seven-token custom theme, no new axes. */
const V1_STORED = {
  mode: "dark",
  preset: "amber",
  radius: "0.75rem",
  custom: { light: V1_LIGHT, dark: V1_DARK },
};

describe("normalizeThemePreference — tone-widening migration", () => {
  it("keeps a v1 seven-token custom palette and DERIVES the widened tones (no fail-closed drop)", () => {
    const norm = normalizeThemePreference(V1_STORED);
    expect(norm.custom).toBeDefined();
    // The seven required tokens are preserved verbatim…
    expect(norm.custom!.dark.accent).toBe("176 68% 50%");
    expect(norm.custom!.light.bg).toBe("40 30% 96%");
    // …and the three widened tones are filled by derivation, matching the pure deriver.
    const expectedDark = deriveTones(V1_DARK);
    expect(norm.custom!.dark.panel2).toBe(expectedDark.panel2);
    expect(norm.custom!.dark.overlay).toBe(expectedDark.overlay);
    expect(norm.custom!.dark.accent2).toBe(expectedDark.accent2);
    expect(norm.custom!.light.accent2).toBe(deriveTones(V1_LIGHT).accent2);
  });

  it("drops a custom theme missing a REQUIRED token (fail-closed, no partial apply)", () => {
    const broken = {
      ...V1_STORED,
      custom: { light: { ...V1_LIGHT, accent: undefined }, dark: V1_DARK },
    };
    const norm = normalizeThemePreference(broken as unknown);
    expect(norm.custom).toBeUndefined(); // dropped whole, not partially applied
    // the rest of the preference still normalizes (preset/radius survive)
    expect(norm.preset).toBe("amber");
    expect(norm.radius).toBe("0.75rem");
  });

  it("falls a non-object stored value to DEFAULT_THEME", () => {
    expect(normalizeThemePreference("garbage")).toEqual(DEFAULT_THEME);
    expect(normalizeThemePreference(null)).toEqual(DEFAULT_THEME);
  });

  it("preserves a preference with already-widened tones untouched", () => {
    const widened = {
      ...V1_STORED,
      custom: {
        light: { ...V1_LIGHT, panel2: "1 1% 1%", overlay: "2 2% 2%", accent2: "3 3% 3%" },
        dark: { ...V1_DARK, panel2: "4 4% 4%", overlay: "5 5% 5%", accent2: "6 6% 6%" },
      },
    };
    const norm = normalizeThemePreference(widened);
    expect(norm.custom!.light.panel2).toBe("1 1% 1%");
    expect(norm.custom!.dark.accent2).toBe("6 6% 6%");
  });
});

// The shell-chrome-layout axes (header style + nav mode). The load-bearing case: a stored theme that
// PREDATES these fields (every existing ui_theme blob) normalizes to the current look (band/sidebar)
// so no existing user flips layout on next load — the migration-safety guarantee.
describe("normalizeThemePreference — header/nav layout axes", () => {
  it("fills header/nav from DEFAULT_LAYOUT when absent (an old stored theme stays put)", () => {
    const old = { mode: "dark", preset: "teal", radius: "0.5rem", layout: { variant: "sidebar", collapsible: "icon", side: "left" } };
    const norm = normalizeThemePreference(old);
    expect(norm.layout.header).toBe(DEFAULT_LAYOUT.header);
    expect(norm.layout.nav).toBe(DEFAULT_LAYOUT.nav);
    // the other axes it DID carry are preserved
    expect(norm.layout.variant).toBe("sidebar");
  });

  it("rejects unknown header/nav values (fails closed to the default, never partial)", () => {
    const bad = { mode: "dark", preset: "teal", radius: "0.5rem", layout: { header: "tabs", nav: "ribbon" } };
    const norm = normalizeThemePreference(bad);
    expect(norm.layout.header).toBe(DEFAULT_LAYOUT.header);
    expect(norm.layout.nav).toBe(DEFAULT_LAYOUT.nav);
  });

  it("preserves explicitly-set header/nav values (breadcrumbs/topmenu round-trip through normalize)", () => {
    const set = { mode: "dark", preset: "teal", radius: "0.5rem", layout: { header: "breadcrumbs", nav: "topmenu" } };
    const norm = normalizeThemePreference(set);
    expect(norm.layout.header).toBe("breadcrumbs");
    expect(norm.layout.nav).toBe("topmenu");
  });

  it("keeps the other layout axes when only header/nav are malformed", () => {
    const mixed = {
      mode: "dark",
      preset: "teal",
      radius: "0.5rem",
      layout: { variant: "floating", collapsible: "offcanvas", side: "right", header: "garbage" },
    };
    const norm = normalizeThemePreference(mixed);
    expect(norm.layout.variant).toBe("floating");
    expect(norm.layout.collapsible).toBe("offcanvas");
    expect(norm.layout.side).toBe("right");
    expect(norm.layout.header).toBe(DEFAULT_LAYOUT.header); // the bad one fell back
    expect(norm.layout.nav).toBe(DEFAULT_LAYOUT.nav); // absent → default
  });
});
