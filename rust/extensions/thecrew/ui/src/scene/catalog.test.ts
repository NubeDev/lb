// catalog.ts unit tests (graphics-canvas phase 3): the shape catalog derived from the LIVE symbol
// registry (one source of truth) — so a teaching error + the SKILL doc describe exactly what renders.

import { describe, expect, it } from "vitest";
import { describeCatalog, knownTypes, catalogText } from "./catalog";
import { SYMBOLS } from "../canvas/ShapeNode";

describe("shape catalog", () => {
  it("stays in sync with the renderer's SYMBOLS registry (the two def lists must not drift)", () => {
    // catalog builds from the def imports directly (to avoid a cycle); this asserts that list matches
    // exactly what ShapeNode renders, so a teaching error can never cite a type the canvas can't draw.
    expect(describeCatalog().map((e) => e.type).sort()).toEqual(Object.keys(SYMBOLS).sort());
  });

  it("carries each type's authorable props + bind slots (from the def's schema)", () => {
    const fan = describeCatalog().find((e) => e.type === "hvac.fan");
    expect(fan?.label).toBe("Fan");
    expect(fan?.props.direction).toEqual({ kind: "select", options: ["left", "right"] });
    expect(fan?.bindSlots).toContain("speed");
  });

  it("knownTypes matches the registry keys (what an unknown-type error checks against)", () => {
    expect(knownTypes()).toEqual(Object.keys(SYMBOLS));
  });

  it("catalogText renders one teachable line per type with props + bind", () => {
    const text = catalogText();
    expect(text).toContain("hvac.fan — props:");
    expect(text).toContain("direction(select: left|right)");
    expect(text).toContain("bind: running, speed, fault");
  });
});
