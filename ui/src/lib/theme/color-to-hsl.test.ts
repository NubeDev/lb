import { describe, expect, it } from "vitest";

import { colorToHslTriplet } from "./color-to-hsl";

describe("colorToHslTriplet", () => {
  it("converts hex (3 and 6 digit)", () => {
    expect(colorToHslTriplet("#ffffff")).toBe("0 0% 100%");
    expect(colorToHslTriplet("#000000")).toBe("0 0% 0%");
    expect(colorToHslTriplet("#fff")).toBe("0 0% 100%");
    expect(colorToHslTriplet("#3b82f6")).toBe("217 91% 60%");
  });

  it("passes hsl(...) through as a triplet (ignoring alpha)", () => {
    expect(colorToHslTriplet("hsl(217 91% 60%)")).toBe("217 91% 60%");
    expect(colorToHslTriplet("hsl(217, 91%, 60%)")).toBe("217 91% 60%");
    expect(colorToHslTriplet("hsla(217 91% 60% / 0.5)")).toBe("217 91% 60%");
  });

  it("converts oklch to a plausible hsl triplet", () => {
    // oklch white / black — lightness at the extremes.
    expect(colorToHslTriplet("oklch(1 0 0)")).toMatch(/% (99|100)%$/);
    expect(colorToHslTriplet("oklch(0 0 0)")).toBe("0 0% 0%");
  });

  it("returns null on unparseable input", () => {
    expect(colorToHslTriplet("rebeccapurple")).toBeNull();
    expect(colorToHslTriplet("#xyz")).toBeNull();
    expect(colorToHslTriplet("")).toBeNull();
  });
});
