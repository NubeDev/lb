import { describe, expect, it } from "vitest";

import { adaptPreset } from "./preset-adapter";
import type { ThemePreset } from "./theme-preset";

// The token bridge is the feature (theme-customizer-scope: "the adapter + its round-trip test are the
// load-bearing piece; treat a missing/weak adapter test as a blocker"). These prove a shadcn-vocabulary
// preset maps to the correct BASE tokens for both light and dark, so charts/panels re-theme.

/** A preset whose light `primary` is a known hex, so we can assert the adapted `accent` triplet exactly. */
const PRESET: ThemePreset = {
  label: "Test",
  styles: {
    light: {
      background: "#ffffff", // → bg 0 0% 100%
      foreground: "#000000", // → fg 0 0% 0%
      card: "#f0f0f0",
      popover: "#e0e0e0",
      primary: "#3b82f6", // → accent (blue-500)
      muted: "#6b7280",
      "muted-foreground": "#9ca3af",
      border: "#e5e7eb",
      input: "#d1d5db",
      ring: "#3b82f6",
    },
    dark: {
      background: "#000000",
      foreground: "#ffffff",
      card: "#111111",
      popover: "#181818",
      primary: "#60a5fa",
      muted: "#9ca3af",
      "muted-foreground": "#6b7280",
      border: "#1f2937",
    },
  },
};

describe("preset adapter (shadcn preset → base tokens)", () => {
  it("maps shadcn vars back onto base tokens for light and dark", () => {
    const theme = adaptPreset(PRESET);
    expect(theme).not.toBeNull();
    const t = theme!;

    // --background → --bg, --foreground → --fg
    expect(t.light.bg).toBe("0 0% 100%");
    expect(t.light.fg).toBe("0 0% 0%");
    expect(t.dark.bg).toBe("0 0% 0%");
    expect(t.dark.fg).toBe("0 0% 100%");

    // --card → --panel (card wins over popover)
    expect(t.light.panel).toBe("0 0% 94%"); // #f0f0f0
    // --primary → --accent (blue-500 #3b82f6 ≈ 217 91% 60%)
    expect(t.light.accent).toBe("217 91% 60%");
    // --muted → --muted, --muted-foreground → --muted-foreground
    expect(t.light.muted).toBe("220 9% 46%"); // #6b7280
    expect(t.light.mutedForeground).toBe("218 11% 65%"); // #9ca3af
    // --border → --border
    expect(t.light.border).toBe("220 13% 91%"); // #e5e7eb
  });

  it("accepts oklch and hsl source colors (not just hex)", () => {
    const theme = adaptPreset({
      label: "Mixed",
      styles: {
        light: {
          background: "oklch(1 0 0)", // white
          foreground: "hsl(0 0% 0%)", // black
          card: "oklch(0.97 0 0)",
          primary: "hsl(217 91% 60%)",
          border: "#e5e7eb",
        },
        dark: {
          background: "oklch(0.145 0 0)",
          foreground: "oklch(0.985 0 0)",
          card: "oklch(0.205 0 0)",
          primary: "oklch(0.7 0.15 250)",
          border: "oklch(0.3 0 0)",
        },
      },
    });
    expect(theme).not.toBeNull();
    // white background → very high lightness
    expect(theme!.light.bg).toMatch(/% (99|100)%$/);
    // hsl primary passes through as its triplet
    expect(theme!.light.accent).toBe("217 91% 60%");
  });

  it("returns null when a mode is missing an identity token (bg/fg/accent)", () => {
    const bad = adaptPreset({
      label: "Broken",
      styles: {
        light: { border: "#e5e7eb" }, // no background/foreground/primary
        dark: { background: "#000", foreground: "#fff", primary: "#60a5fa" },
      },
    });
    expect(bad).toBeNull();
  });

  it("completes a partial palette by borrowing neighbors (no crash on a sparse preset)", () => {
    const theme = adaptPreset({
      label: "Sparse",
      styles: {
        light: { background: "#ffffff", foreground: "#000000", primary: "#3b82f6" },
        dark: { background: "#000000", foreground: "#ffffff", primary: "#60a5fa" },
      },
    });
    expect(theme).not.toBeNull();
    // panel/muted/mutedForeground/border all present (borrowed), so the palette is fully applyable.
    for (const key of ["bg", "panel", "fg", "muted", "mutedForeground", "accent", "border"] as const) {
      expect(theme!.light[key]).toBeTruthy();
      expect(theme!.dark[key]).toBeTruthy();
    }
  });
});
