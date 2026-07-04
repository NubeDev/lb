import { describe, expect, it } from "vitest";

import { completeTones, deriveTones } from "./derive-tones";
import { parseTriplet } from "./hsl-triplet";
import type { RequiredPalette } from "./theme-tokens";

const DARK: RequiredPalette = {
  bg: "24 8% 8%",
  panel: "24 7% 11%",
  fg: "36 16% 90%",
  muted: "30 5% 56%",
  mutedForeground: "30 5% 60%",
  accent: "34 96% 58%",
  border: "24 6% 18%",
};
const LIGHT: RequiredPalette = {
  bg: "40 30% 96%",
  panel: "40 24% 92%",
  fg: "30 12% 14%",
  muted: "30 6% 44%",
  mutedForeground: "30 6% 40%",
  accent: "32 92% 34%",
  border: "40 12% 82%",
};

describe("deriveTones", () => {
  it("steps raised surfaces TOWARD the foreground per mode (lighter in dark, darker in light)", () => {
    const dark = deriveTones(DARK);
    // dark: panel2 lighter than panel (11 -> 15)
    expect(parseTriplet(dark.panel2)!.l).toBeGreaterThan(parseTriplet(DARK.panel)!.l);
    expect(parseTriplet(dark.overlay)!.l).toBeGreaterThan(parseTriplet(DARK.bg)!.l);

    const light = deriveTones(LIGHT);
    // light: panel2 darker than panel (92 -> 88)
    expect(parseTriplet(light.panel2)!.l).toBeLessThan(parseTriplet(LIGHT.panel)!.l);
    expect(parseTriplet(light.overlay)!.l).toBeLessThan(parseTriplet(LIGHT.bg)!.l);
  });

  it("rotates the secondary accent's hue off the accent", () => {
    const { accent2 } = deriveTones(DARK);
    expect(parseTriplet(accent2)!.h).not.toBe(parseTriplet(DARK.accent)!.h);
    // saturation/lightness preserved (only hue shifts)
    expect(parseTriplet(accent2)!.s).toBe(parseTriplet(DARK.accent)!.s);
    expect(parseTriplet(accent2)!.l).toBe(parseTriplet(DARK.accent)!.l);
  });

  it("completeTones keeps present tones and derives only the absent ones", () => {
    const kept = completeTones(DARK, { panel2: "1 2% 3%" });
    expect(kept.panel2).toBe("1 2% 3%"); // preserved
    expect(kept.overlay).toBe(deriveTones(DARK).overlay); // derived
    expect(kept.accent2).toBe(deriveTones(DARK).accent2); // derived
    // an empty string is treated as absent (derived, not kept)
    expect(completeTones(DARK, { panel2: "" }).panel2).toBe(deriveTones(DARK).panel2);
  });
});
