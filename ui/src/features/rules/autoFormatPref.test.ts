// Unit tests for the auto-format preference store (rules-editor-ux). Load-bearing properties: the
// flag round-trips through localStorage, the default is OFF (opt-in), and a missing/legacy value
// reads as OFF. A fake storage is passed in (no real localStorage needed).

import { describe, expect, it } from "vitest";

import { AUTO_FORMAT_KEY, readAutoFormat, writeAutoFormat, type LocalStorage } from "./autoFormatPref";

function fakeStorage(seed: Record<string, string> = {}): LocalStorage & { map: Map<string, string> } {
  const map = new Map(Object.entries(seed));
  return {
    map,
    getItem: (k) => map.get(k) ?? null,
    setItem: (k, v) => void map.set(k, v),
  };
}

describe("autoFormatPref", () => {
  it("defaults to false when nothing is stored", () => {
    expect(readAutoFormat(fakeStorage())).toBe(false);
  });

  it("round-trips true", () => {
    const s = fakeStorage();
    writeAutoFormat(true, s);
    expect(s.map.get(AUTO_FORMAT_KEY)).toBe("1");
    expect(readAutoFormat(s)).toBe(true);
  });

  it("round-trips false", () => {
    const s = fakeStorage({ [AUTO_FORMAT_KEY]: "1" });
    writeAutoFormat(false, s);
    expect(readAutoFormat(s)).toBe(false);
  });

  it("reads any non-\"1\" value as false", () => {
    expect(readAutoFormat(fakeStorage({ [AUTO_FORMAT_KEY]: "true" }))).toBe(false);
  });

  it("is a no-op (does not throw) when storage is explicitly unavailable", () => {
    // Simulate an SSR / no-storage environment by passing a nullish storage past the default param.
    const absent = null as unknown as LocalStorage;
    expect(() => writeAutoFormat(true, absent)).not.toThrow();
    expect(readAutoFormat(absent)).toBe(false);
  });
});
