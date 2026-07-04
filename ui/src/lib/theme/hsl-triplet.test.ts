import { describe, expect, it } from "vitest";

import { formatTriplet, hslToHex, parseTriplet, tripletToCss } from "./hsl-triplet";

describe("hsl-triplet", () => {
  it("parses a well-formed triplet and rejects garbage", () => {
    expect(parseTriplet("32 92% 34%")).toEqual({ h: 32, s: 92, l: 34 });
    expect(parseTriplet("210 5.5% 12.5%")).toEqual({ h: 210, s: 5.5, l: 12.5 });
    expect(parseTriplet("#ffffff")).toBeNull();
    expect(parseTriplet("32 92 34")).toBeNull();
    expect(parseTriplet("")).toBeNull();
  });

  it("formats channels back to a rounded, clamped triplet", () => {
    expect(formatTriplet({ h: 32.4, s: 91.6, l: 34.1 })).toBe("32 92% 34%");
    // wraps hue, clamps s/l into range
    expect(formatTriplet({ h: 400, s: 120, l: -5 })).toBe("40 100% 0%");
    expect(formatTriplet({ h: -30, s: 50, l: 50 })).toBe("330 50% 50%");
  });

  it("converts HSL channels to hex", () => {
    expect(hslToHex({ h: 0, s: 0, l: 100 })).toBe("#ffffff");
    expect(hslToHex({ h: 0, s: 0, l: 0 })).toBe("#000000");
    expect(hslToHex({ h: 0, s: 100, l: 50 })).toBe("#ff0000");
    expect(hslToHex({ h: 120, s: 100, l: 50 })).toBe("#00ff00");
    expect(hslToHex({ h: 240, s: 100, l: 50 })).toBe("#0000ff");
  });

  it("round-trips triplet → hex → triplet closely (integer rounding)", () => {
    const t = "210 60% 45%";
    const hex = hslToHex(parseTriplet(t)!);
    expect(hex).toMatch(/^#[0-9a-f]{6}$/);
  });

  it("wraps a triplet into a CSS hsl() string, transparent on garbage", () => {
    expect(tripletToCss("32 92% 34%")).toBe("hsl(32 92% 34%)");
    expect(tripletToCss("bogus")).toBe("transparent");
  });
});
