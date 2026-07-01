// Unit test for the ONE field-presentation resolver (widget-kit scope) — proves both declaration sites
// (form `x-lb` label/description/hide/order AND response `fieldConfig` displayName/description/hide)
// resolve through the SAME code path, and that an unhinted field falls back to humanize. Pure logic.

import { describe, expect, it } from "vitest";

import { resolveFieldPresentation } from "./resolve";

describe("resolveFieldPresentation (one resolver, both surfaces)", () => {
  it("uses a `label` override (the form `x-lb` site) over humanize", () => {
    const p = resolveFieldPresentation("maxRuns", { label: "Max Runs", description: "Stop after N" });
    expect(p.label).toBe("Max Runs");
    expect(p.description).toBe("Stop after N");
    expect(p.hidden).toBe(false);
  });

  it("accepts `displayName` (the fieldConfig alias) as the label — same result as `label`", () => {
    // The response TABLE reads Grafana `fieldConfig.displayName`; it must resolve identically to a form
    // `label` so a header and a form label never drift.
    const fromForm = resolveFieldPresentation("maxRuns", { label: "Max Runs" });
    const fromTable = resolveFieldPresentation("maxRuns", { displayName: "Max Runs" });
    expect(fromTable.label).toBe(fromForm.label);
  });

  it("falls back to humanize when no label/displayName is declared", () => {
    expect(resolveFieldPresentation("nextAttemptTs", undefined).label).toBe("Next Attempt Ts");
    expect(resolveFieldPresentation("principalSub", {}).label).toBe("Principal Sub");
  });

  it("reflects `hide` as hidden (presentation, not security)", () => {
    expect(resolveFieldPresentation("principalSub", { hide: true }).hidden).toBe(true);
    expect(resolveFieldPresentation("id", { hide: false }).hidden).toBe(false);
    expect(resolveFieldPresentation("id", {}).hidden).toBe(false);
  });

  it("passes `order` through as an optional override (absent → undefined, never implicit)", () => {
    expect(resolveFieldPresentation("a", { order: 5 }).order).toBe(5);
    expect(resolveFieldPresentation("a", {}).order).toBeUndefined();
  });
});
