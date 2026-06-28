import { describe, expect, it, vi } from "vitest";

import { DEFAULT_THEME, THEME_STORAGE_KEY } from "./theme-options";
import { loadThemePreference, saveThemePreference } from "./theme-storage";

class MemoryThemeStorage {
  values = new Map<string, string>();

  getItem(key: string) {
    return this.values.get(key) ?? null;
  }

  setItem(key: string, value: string) {
    this.values.set(key, value);
  }
}

describe("theme storage", () => {
  it("loads a valid saved preference", () => {
    const storage = new MemoryThemeStorage();
    storage.setItem(THEME_STORAGE_KEY, JSON.stringify({ mode: "light", accent: "teal" }));

    expect(loadThemePreference(storage)).toEqual({ mode: "light", accent: "teal" });
  });

  it("normalizes unknown values to defaults", () => {
    const storage = new MemoryThemeStorage();
    storage.setItem(THEME_STORAGE_KEY, JSON.stringify({ mode: "solarized", accent: "pink" }));

    expect(loadThemePreference(storage)).toEqual(DEFAULT_THEME);
  });

  it("falls back when storage is unavailable or malformed", () => {
    const storage = new MemoryThemeStorage();
    storage.setItem(THEME_STORAGE_KEY, "{not-json");

    expect(loadThemePreference(storage)).toEqual(DEFAULT_THEME);
    expect(loadThemePreference(undefined)).toEqual(DEFAULT_THEME);
  });

  it("ignores write failures", () => {
    const storage = {
      getItem: vi.fn(),
      setItem: vi.fn(() => {
        throw new Error("blocked");
      }),
    };

    expect(() => saveThemePreference({ mode: "dark", accent: "blue" }, storage)).not.toThrow();
  });
});
