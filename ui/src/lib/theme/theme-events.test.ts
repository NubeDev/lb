// The theme change emitter — exactly one signal per applied change, one fan-out point (the ext-host
// subscriber contract). Proves: applying a theme fires `lb:themechange` exactly once; a subscriber gets
// it; unsubscribe stops delivery.

import { afterEach, describe, expect, it, vi } from "vitest";

import { emitThemeChange, onThemeChange } from "./theme-events";
import { applyThemePreference } from "./theme-dom";
import { DEFAULT_THEME } from "./theme-options";

afterEach(() => vi.restoreAllMocks());

describe("theme-events", () => {
  it("delivers a change to a subscriber and stops after unsubscribe", () => {
    const fn = vi.fn();
    const off = onThemeChange(fn);
    emitThemeChange();
    expect(fn).toHaveBeenCalledTimes(1);
    emitThemeChange();
    expect(fn).toHaveBeenCalledTimes(2);
    off();
    emitThemeChange();
    expect(fn).toHaveBeenCalledTimes(2); // no more after unsubscribe
  });

  it("fires EXACTLY ONCE per applyThemePreference (one emitter, one fan-out)", () => {
    const doc = document.implementation.createHTMLDocument("theme");
    const fn = vi.fn();
    const off = onThemeChange(fn);
    applyThemePreference(doc, DEFAULT_THEME);
    expect(fn).toHaveBeenCalledTimes(1); // not per-token, not per-attribute — one change, one signal
    off();
  });

  it("fans out to every subscriber (the single ext-host is one of N in tests)", () => {
    const a = vi.fn();
    const b = vi.fn();
    const offA = onThemeChange(a);
    const offB = onThemeChange(b);
    emitThemeChange();
    expect(a).toHaveBeenCalledTimes(1);
    expect(b).toHaveBeenCalledTimes(1);
    offA();
    offB();
  });
});
