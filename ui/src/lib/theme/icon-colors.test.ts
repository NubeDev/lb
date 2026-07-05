// icon-colors.data — the palette + auto-assignment math (pure, no DOM). Guards the contract the
// icon-colorizer UI and NavRail application both depend on: 100 distinct prefilled colors, even
// sampling for any key set, hex validation that fails closed, and stable deterministic assignment.

import { describe, expect, it } from "vitest";

import {
  ICON_COLOR_PALETTE,
  ICON_COLOR_PALETTE_SIZE,
  autoAssignIconColors,
  isValidHex,
  normalizeHex,
} from "./icon-colors.data";

describe("icon-colors palette", () => {
  it("ships exactly the prefilled 100 colors", () => {
    expect(ICON_COLOR_PALETTE).toHaveLength(ICON_COLOR_PALETTE_SIZE);
    expect(ICON_COLOR_PALETTE_SIZE).toBe(100);
  });

  it("every palette entry is a canonical lowercase #rrggbb hex", () => {
    for (const c of ICON_COLOR_PALETTE) {
      expect(isValidHex(c)).toBe(true);
      expect(c).toBe(c.toLowerCase());
    }
  });

  it("palette entries are unique (no duplicate colors)", () => {
    expect(new Set(ICON_COLOR_PALETTE).size).toBe(ICON_COLOR_PALETTE_SIZE);
  });

  it("the palette is frozen data — callers cannot mutate the source", () => {
    expect(Object.isFrozen(ICON_COLOR_PALETTE)).toBe(true);
  });
});

describe("autoAssignIconColors", () => {
  it("returns an empty map for no keys", () => {
    expect(autoAssignIconColors([])).toEqual({});
  });

  it("assigns exactly one color per key, every value a palette member", () => {
    const keys = ["channels", "dashboards", "inbox", "rules", "admin"];
    const assigned = autoAssignIconColors(keys);
    expect(Object.keys(assigned).sort()).toEqual([...keys].sort());
    for (const v of Object.values(assigned)) {
      expect(ICON_COLOR_PALETTE).toContain(v);
    }
  });

  it("is deterministic — the same keys always map to the same colors", () => {
    const keys = ["a", "b", "c", "d", "e"];
    expect(autoAssignIconColors(keys)).toEqual(autoAssignIconColors(keys));
  });

  it("spans the palette: a small key set samples colors far apart, not the first N neighbors", () => {
    const three = Object.values(autoAssignIconColors(["a", "b", "c"]));
    // Three sampled indices should be evenly spread (~0, ~33, ~67) — never three clustered neighbors.
    const indices = three.map((c) => ICON_COLOR_PALETTE.indexOf(c)).sort((x, y) => x - y);
    expect(indices[0]).toBeLessThan(5);
    expect(indices[1] - indices[0]).toBeGreaterThan(10);
    expect(indices[2] - indices[1]).toBeGreaterThan(10);
    // And reusing the palette for a larger key set keeps every assignment distinct.
    const many = new Set(Object.values(autoAssignIconColors(Array.from({ length: 20 }, (_, i) => `k${i}`))));
    expect(many.size).toBe(20);
  });
});

describe("hex validation", () => {
  it("accepts canonical #rrggbb", () => {
    expect(isValidHex("#ff8800")).toBe(true);
    expect(isValidHex("#ABCDEF")).toBe(true);
  });

  it("rejects non-hex or malformed values (fail closed)", () => {
    expect(isValidHex("#ff880")).toBe(false); // too short
    expect(isValidHex("#ff88000")).toBe(false); // too long
    expect(isValidHex("ff8800")).toBe(false); // missing #
    expect(isValidHex("#gghhii")).toBe(false); // non-hex chars
    expect(isValidHex(null)).toBe(false);
    expect(isValidHex(undefined)).toBe(false);
    expect(isValidHex(123)).toBe(false);
  });

  it("normalizes shorthand #rgb to canonical lowercase #rrggbb", () => {
    expect(normalizeHex("#f80")).toBe("#ff8800");
    expect(normalizeHex("#ABC")).toBe("#aabbcc");
    expect(normalizeHex("#AaBbCc")).toBe("#aabbcc");
  });

  it("normalize fails closed to null for garbage", () => {
    expect(normalizeHex("not a color")).toBeNull();
    expect(normalizeHex("#ff")).toBeNull();
    expect(normalizeHex(null)).toBeNull();
    expect(normalizeHex({} as unknown)).toBeNull();
  });
});
