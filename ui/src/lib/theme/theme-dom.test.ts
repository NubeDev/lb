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
  },
  dark: {
    bg: "24 8% 8%",
    panel: "24 7% 11%",
    fg: "36 16% 90%",
    muted: "30 5% 56%",
    mutedForeground: "30 5% 60%",
    accent: "176 68% 50%",
    border: "24 6% 18%",
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
});
