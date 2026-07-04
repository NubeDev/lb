import { describe, expect, it } from "vitest";

import { applyThemePreference } from "./theme-dom";
import { DEFAULT_THEME, type ThemePreference } from "./theme-options";
import type { CustomTheme } from "./theme-tokens";

/** Build a full ThemePreference from a partial (layout etc. filled from the default) — the DOM writer
 *  ignores `layout`, so tests only vary mode/preset/radius/custom. */
const pref = (p: Partial<ThemePreference>): ThemePreference => ({ ...DEFAULT_THEME, ...p });

const PALETTE: CustomTheme = {
  light: {
    bg: "40 30% 96%",
    panel: "40 24% 92%",
    fg: "30 12% 14%",
    muted: "30 6% 44%",
    mutedForeground: "30 6% 40%",
    accent: "178 70% 28%",
    border: "40 12% 82%",
    panel2: "40 26% 88%",
    overlay: "30 10% 20%",
    accent2: "218 60% 32%",
  },
  dark: {
    bg: "24 8% 8%",
    panel: "24 7% 11%",
    fg: "36 16% 90%",
    muted: "30 5% 56%",
    mutedForeground: "30 5% 60%",
    accent: "176 68% 50%",
    border: "24 6% 18%",
    panel2: "24 7% 15%",
    overlay: "36 12% 82%",
    accent2: "216 62% 52%",
  },
};

describe("theme DOM application", () => {
  it("applies dark mode, radius, and the built-in accent attribute (no inline base tokens)", () => {
    const doc = document.implementation.createHTMLDocument("theme");

    applyThemePreference(doc, pref({ mode: "dark", preset: "teal", radius: "0.75rem" }));

    const root = doc.documentElement;
    expect(root.classList.contains("dark")).toBe(true);
    expect(root.dataset.themeAccent).toBe("teal");
    expect(root.style.colorScheme).toBe("dark");
    expect(root.style.getPropertyValue("--radius")).toBe("0.75rem");
    // A built-in accent applies via the attribute — NOT inline base tokens.
    expect(root.style.getPropertyValue("--accent")).toBe("");
  });

  it("removes dark mode for light preferences", () => {
    const doc = document.implementation.createHTMLDocument("theme");
    doc.documentElement.classList.add("dark");

    applyThemePreference(doc, pref({ mode: "light", preset: "blue", radius: "0.5rem" }));

    expect(doc.documentElement.classList.contains("dark")).toBe(false);
    expect(doc.documentElement.dataset.themeAccent).toBe("blue");
    expect(doc.documentElement.style.colorScheme).toBe("light");
  });

  it("writes inline BASE tokens for a custom theme and clears the accent attribute", () => {
    const doc = document.implementation.createHTMLDocument("theme");
    doc.documentElement.dataset.themeAccent = "amber"; // a stale attr from a prior built-in theme

    applyThemePreference(doc, pref({ mode: "dark", preset: "amber", radius: "0.5rem", custom: PALETTE }));

    const root = doc.documentElement;
    // The active (dark) palette is written inline as base tokens…
    expect(root.style.getPropertyValue("--accent")).toBe("176 68% 50%");
    expect(root.style.getPropertyValue("--bg")).toBe("24 8% 8%");
    expect(root.style.getPropertyValue("--fg")).toBe("36 16% 90%");
    // …and the built-in attribute is cleared so it can't fight the inline value.
    expect(root.dataset.themeAccent).toBeUndefined();
  });

  it("re-applies the correct light/dark variant of a custom theme on mode flip", () => {
    const doc = document.implementation.createHTMLDocument("theme");

    applyThemePreference(doc, pref({ mode: "dark", preset: "amber", radius: "0.5rem", custom: PALETTE }));
    expect(doc.documentElement.style.getPropertyValue("--bg")).toBe("24 8% 8%"); // dark

    applyThemePreference(doc, pref({ mode: "light", preset: "amber", radius: "0.5rem", custom: PALETTE }));
    expect(doc.documentElement.style.getPropertyValue("--bg")).toBe("40 30% 96%"); // light variant
  });

  it("clears inline base tokens when switching from a custom theme back to a built-in accent", () => {
    const doc = document.implementation.createHTMLDocument("theme");

    applyThemePreference(doc, pref({ mode: "dark", preset: "amber", radius: "0.5rem", custom: PALETTE }));
    expect(doc.documentElement.style.getPropertyValue("--accent")).not.toBe("");

    applyThemePreference(doc, pref({ mode: "dark", preset: "amber", radius: "0.5rem" }));
    // Inline base tokens removed; the static :root/.dark blocks own the values again.
    expect(doc.documentElement.style.getPropertyValue("--accent")).toBe("");
    expect(doc.documentElement.dataset.themeAccent).toBe("amber");
  });

  it("writes data-surface, data-motion, and the font-stack tokens from the resolved appearance", () => {
    const doc = document.implementation.createHTMLDocument("theme");

    applyThemePreference(doc, pref({ look: "glass", surface: "elevated", motion: "full", fontSans: "inter", fontMono: "jetbrains-mono" }));

    const root = doc.documentElement;
    expect(root.dataset.surface).toBe("elevated"); // explicit override wins over the glass look
    expect(root.dataset.motion).toBe("full"); // explicit full survives (also overrides reduced-motion)
    expect(root.style.getPropertyValue("--font-sans")).toContain('"Inter"');
    expect(root.style.getPropertyValue("--font-mono")).toContain('"JetBrains Mono"');
  });

  it("falls the surface/motion/fonts through to the look's defaults when unset", () => {
    const doc = document.implementation.createHTMLDocument("theme");

    applyThemePreference(doc, pref({ look: "glass" }));

    const root = doc.documentElement;
    expect(root.dataset.surface).toBe("glass"); // the glass look's default surface
    expect(root.dataset.glass).toBe("medium"); // the glass look lands at medium intensity
    expect(root.style.getPropertyValue("--font-sans")).toContain('"Inter"'); // glass defaults to inter
  });

  it("writes data-glass — a member override wins, else the built-in subtle default", () => {
    const doc = document.implementation.createHTMLDocument("theme");
    // A member override on a non-glass look still writes the attribute (CSS only reads it under glass).
    applyThemePreference(doc, pref({ look: "default", glass: "heavy" }));
    expect(doc.documentElement.dataset.glass).toBe("heavy");
    // With no override and no look default, glass falls to the built-in `subtle`.
    const doc2 = document.implementation.createHTMLDocument("theme");
    applyThemePreference(doc2, pref({ look: "default" }));
    expect(doc2.documentElement.dataset.glass).toBe("subtle");
  });

  it("radius follows the resolved appearance (a look's stamped radius)", () => {
    const doc = document.implementation.createHTMLDocument("theme");
    // A preference where the member picked the modern look (applyLook stamped radius 1rem).
    applyThemePreference(doc, pref({ look: "modern", radius: "1rem" }));
    expect(doc.documentElement.style.getPropertyValue("--radius")).toBe("1rem");
  });
});
