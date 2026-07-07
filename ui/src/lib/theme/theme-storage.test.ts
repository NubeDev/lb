import { describe, expect, it, vi } from "vitest";

import { DEFAULT_LAYOUT, DEFAULT_THEME, THEME_STORAGE_KEY } from "./theme-options";
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
    storage.setItem(THEME_STORAGE_KEY, JSON.stringify({ mode: "light", preset: "teal", radius: "0.75rem" }));

    expect(loadThemePreference(storage)).toEqual({
      mode: "light",
      preset: "teal",
      radius: "0.75rem",
      look: "default",
      layout: DEFAULT_LAYOUT,
    });
  });

  it("normalizes unknown mode/radius to defaults (keeps an unknown preset id for apply-time resolve)", () => {
    const storage = new MemoryThemeStorage();
    storage.setItem(THEME_STORAGE_KEY, JSON.stringify({ mode: "solarized", preset: "custom-lib", radius: "9rem" }));

    // mode + radius fall back; the preset string is kept (it may be a valid library id).
    expect(loadThemePreference(storage)).toEqual({
      mode: DEFAULT_THEME.mode,
      preset: "custom-lib",
      radius: DEFAULT_THEME.radius,
      look: "default",
      layout: DEFAULT_LAYOUT,
    });
  });

  it("normalizes a legacy {mode, accent} record to the default (no compat upgrade)", () => {
    const storage = new MemoryThemeStorage();
    storage.setItem(THEME_STORAGE_KEY, JSON.stringify({ mode: "light", accent: "teal" }));

    // The old shape has no `preset`/`radius`/`layout`; normalize fills them with defaults (accent dropped).
    expect(loadThemePreference(storage)).toEqual({
      mode: "light",
      preset: "teal",
      radius: "0.5rem",
      look: "default",
      layout: DEFAULT_LAYOUT,
    });
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

    expect(() =>
      saveThemePreference({ mode: "dark", preset: "blue", radius: "0.5rem", look: "default", layout: DEFAULT_LAYOUT }, storage),
    ).not.toThrow();
  });
});
