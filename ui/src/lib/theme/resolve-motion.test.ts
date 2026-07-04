// The motion fold — `prefers-reduced-motion` forces `off` UNLESS the member explicitly chose `full`.

import { afterEach, describe, expect, it, vi } from "vitest";

import { resolveMotion } from "./resolve-motion";

/** Fake a document whose window.matchMedia reports the given reduced-motion state. */
function docWithReducedMotion(reduce: boolean): Document {
  const doc = document.implementation.createHTMLDocument("motion");
  Object.defineProperty(doc, "defaultView", {
    configurable: true,
    value: { matchMedia: (q: string) => ({ matches: reduce && q.includes("reduce") }) },
  });
  return doc;
}

afterEach(() => vi.restoreAllMocks());

describe("resolveMotion", () => {
  it("passes motion through when reduced-motion is OFF", () => {
    const doc = docWithReducedMotion(false);
    expect(resolveMotion("off", doc)).toBe("off");
    expect(resolveMotion("subtle", doc)).toBe("subtle");
    expect(resolveMotion("full", doc)).toBe("full");
  });

  it("forces subtle/off to off when reduced-motion is ON", () => {
    const doc = docWithReducedMotion(true);
    expect(resolveMotion("subtle", doc)).toBe("off");
    expect(resolveMotion("off", doc)).toBe("off");
  });

  it("keeps an EXPLICIT full even under reduced-motion (informed opt-in)", () => {
    const doc = docWithReducedMotion(true);
    expect(resolveMotion("full", doc)).toBe("full");
  });

  it("treats a missing matchMedia (jsdom/SSR) as no reduced-motion", () => {
    const doc = document.implementation.createHTMLDocument("motion");
    Object.defineProperty(doc, "defaultView", { configurable: true, value: {} });
    expect(resolveMotion("subtle", doc)).toBe("subtle");
  });
});
