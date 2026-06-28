import { describe, expect, it } from "vitest";

import { applyThemePreference } from "./theme-dom";

describe("theme DOM application", () => {
  it("applies dark mode and accent attributes", () => {
    const doc = document.implementation.createHTMLDocument("theme");

    applyThemePreference(doc, { mode: "dark", accent: "teal" });

    expect(doc.documentElement.classList.contains("dark")).toBe(true);
    expect(doc.documentElement.dataset.themeAccent).toBe("teal");
    expect(doc.documentElement.style.colorScheme).toBe("dark");
  });

  it("removes dark mode for light preferences", () => {
    const doc = document.implementation.createHTMLDocument("theme");
    doc.documentElement.classList.add("dark");

    applyThemePreference(doc, { mode: "light", accent: "blue" });

    expect(doc.documentElement.classList.contains("dark")).toBe(false);
    expect(doc.documentElement.dataset.themeAccent).toBe("blue");
    expect(doc.documentElement.style.colorScheme).toBe("light");
  });
});
