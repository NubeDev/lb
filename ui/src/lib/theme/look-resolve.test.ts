// The look resolver — the per-axis fold (theme-appearance scope). Precedence:
//   pinned look axis → explicit member override → look default → built-in default.

import { describe, expect, it } from "vitest";

import { applyLook, resolveAppearance } from "./look-resolve";
import { DEFAULT_MOTION, DEFAULT_SURFACE } from "./appearance-axes";
import { DEFAULT_FONT_MONO, DEFAULT_FONT_SANS } from "./theme-fonts.data";
import { lookById } from "./theme-looks.data";
import { DEFAULT_THEME, type ThemePreference } from "./theme-options";

const pref = (p: Partial<ThemePreference>): ThemePreference => ({ ...DEFAULT_THEME, ...p });

describe("resolveAppearance", () => {
  it("folds a look's defaults into the OPTIONAL axes when the member set no overrides", () => {
    const glass = lookById("glass")!;
    // The OPTIONAL axes (surface/motion/fonts) fall through to the look default when unset.
    // preset/radius are REQUIRED fields (always present), so the resolver keeps the STORED value — a
    // fresh look pick stamps them via applyLook (tested below), it isn't a resolve-time default.
    const r = resolveAppearance(pref({ look: "glass" }));
    expect(r.look).toBe("glass");
    expect(r.surface).toBe(glass.defaults.surface); // "glass"
    expect(r.motion).toBe(glass.defaults.motion); // "full"
    expect(r.fontSans).toBe(glass.defaults.fontSans);
    expect(r.fontMono).toBe(glass.defaults.fontMono);
    // radius stays the stored DEFAULT (applyLook is what stamps a look's radius) — proven in applyLook.
    expect(r.radius).toBe(DEFAULT_THEME.radius);
  });

  it("an explicit member override wins over the look default (unpinned axis)", () => {
    // glass defaults surface:glass, motion:full — the member forces flat + off.
    const r = resolveAppearance(pref({ look: "glass", surface: "flat", motion: "off" }));
    expect(r.surface).toBe("flat");
    expect(r.motion).toBe("off");
    // an axis they DIDN'T override still comes from the look
    expect(r.fontSans).toBe(lookById("glass")!.defaults.fontSans);
  });

  it("a PINNED look axis wins even over an explicit member choice (retro pins preset)", () => {
    // The member explicitly picks a different preset, but retro OWNS its palette.
    const r = resolveAppearance(pref({ look: "retro", preset: "amber" }));
    expect(r.preset).toBe("retro"); // pinned, member ignored
  });

  it("falls an unknown look id to the default look (fail-open to data)", () => {
    const r = resolveAppearance(pref({ look: "does-not-exist" }));
    expect(r.look).toBe("default");
    expect(r.surface).toBe(lookById("default")!.defaults.surface);
  });

  it("falls unset axes with no look default to the built-in default", () => {
    // The `default` look defines every axis; construct a look-less resolution by using a look that
    // omits an axis is not possible in data, so assert the builtin path via the resolver's constants.
    const r = resolveAppearance(pref({ look: "default" }));
    // default look happens to match the builtins for surface/motion:
    expect(r.surface).toBe(DEFAULT_SURFACE);
    expect(r.motion).toBe(DEFAULT_MOTION);
    expect(r.fontSans).toBe(DEFAULT_FONT_SANS);
    expect(r.fontMono).toBe(DEFAULT_FONT_MONO);
  });
});

describe("applyLook", () => {
  it("resets the axes the look defines and clears per-axis overrides", () => {
    // Start from a member who hand-tweaked several axes on top of the default look.
    const tweaked = pref({ look: "default", surface: "glass", motion: "full", fontSans: "inter", radius: "1rem" });
    const next = applyLook(tweaked, "editor");
    const editor = lookById("editor")!;
    expect(next.look).toBe("editor");
    // overrides cleared — the picked look's defaults now show through the resolver
    expect(next.surface).toBeUndefined();
    expect(next.motion).toBeUndefined();
    expect(next.fontSans).toBeUndefined();
    // the look's preset/radius are stamped onto the (required) fields
    expect(next.preset).toBe(editor.defaults.preset);
    expect(next.radius).toBe(editor.defaults.radius);
    // resolving the result yields the editor look's axes
    const r = resolveAppearance(next);
    expect(r.surface).toBe(editor.defaults.surface);
    expect(r.fontMono).toBe(editor.defaults.fontMono);
  });
});
