// The fieldConfig render-path logic (viz field-config scope): thresholds COLOR (not alert), value
// mappings, and the defaults+overrides resolve. Pure unit tests — no gateway, no render.

import { describe, it, expect } from "vitest";

import { activeStep, thresholdColor } from "./thresholds";
import { applyMappings } from "./mappings";
import { resolveFieldOptions } from "./resolve";
import type { ThresholdsConfig, ValueMapping } from "@/lib/dashboard";

describe("thresholds", () => {
  const cfg: ThresholdsConfig = { mode: "absolute", steps: [{ value: null, color: "green" }, { value: 5, color: "red" }] };

  it("the base step (-∞) matches below the first threshold", () => {
    expect(activeStep(2, cfg)?.color).toBe("green");
  });
  it("a value over a step takes that step's color", () => {
    expect(activeStep(7, cfg)?.color).toBe("red");
  });
  it("treats an ABSENT base value as -∞ (the store drops the explicit null)", () => {
    const dropped: ThresholdsConfig = { mode: "absolute", steps: [{ color: "green" } as never, { value: 5, color: "red" }] };
    expect(activeStep(1, dropped)?.color).toBe("green");
  });
  it("resolves to a themed color token, never the raw name", () => {
    expect(thresholdColor(7, cfg)).toMatch(/hsl\(/);
  });
});

describe("value mappings", () => {
  it("a value mapping replaces an exact value", () => {
    const m: ValueMapping[] = [{ type: "value", options: { "1": { text: "On", color: "green" } } }];
    expect(applyMappings(1, m)?.text).toBe("On");
  });
  it("a range mapping matches inclusively (null = ±∞)", () => {
    const m: ValueMapping[] = [{ type: "range", options: { from: 0, to: 10, result: { text: "low" } } }];
    expect(applyMappings(5, m)?.text).toBe("low");
    expect(applyMappings(11, m)).toBeNull();
  });
  it("a special null mapping matches null", () => {
    const m: ValueMapping[] = [{ type: "special", options: { match: "null", result: { text: "—" } } }];
    expect(applyMappings(null, m)?.text).toBe("—");
  });
  it("a regex mapping is deferred (no match), never wrong", () => {
    const m: ValueMapping[] = [{ type: "regex", options: { pattern: ".*", result: { text: "x" } } }];
    expect(applyMappings("abc", m)).toBeNull();
  });
});

describe("resolve defaults + overrides", () => {
  it("a byName override wins over defaults for the matched field", () => {
    const opts = resolveFieldOptions(
      { defaults: { unit: "short", decimals: 0 }, overrides: [{ matcher: { id: "byName", options: "value" }, properties: [{ id: "unit", value: "celsius" }] }] },
      { name: "value", type: "number" },
    );
    expect(opts.unit).toBe("celsius");
    expect(opts.decimals).toBe(0); // unchanged default
  });
  it("a non-matching override leaves defaults intact", () => {
    const opts = resolveFieldOptions(
      { defaults: { unit: "short" }, overrides: [{ matcher: { id: "byName", options: "other" }, properties: [{ id: "unit", value: "celsius" }] }] },
      { name: "value", type: "number" },
    );
    expect(opts.unit).toBe("short");
  });
  it("a dotted custom.* id writes into the custom bag (Grafana ids verbatim)", () => {
    const opts = resolveFieldOptions(
      { defaults: {}, overrides: [{ matcher: { id: "byType", options: "number" }, properties: [{ id: "custom.lineWidth", value: 3 }] }] },
      { name: "value", type: "number" },
    );
    expect(opts.custom?.lineWidth).toBe(3);
  });

  // byRegexp matcher (wired when the editor started authoring it — editor-parity step 4).
  it("a byRegexp matcher applies to fields whose name matches the pattern", () => {
    const cfg = { defaults: {}, overrides: [{ matcher: { id: "byRegexp" as const, options: "cpu.*" }, properties: [{ id: "unit", value: "percent" }] }] };
    expect(resolveFieldOptions(cfg, { name: "cpu_load", type: "number" }).unit).toBe("percent");
    expect(resolveFieldOptions(cfg, { name: "mem_used", type: "number" }).unit).toBeUndefined();
  });
  it("a byRegexp `/pattern/i` literal honors flags; an invalid pattern never matches (no throw)", () => {
    const ci = { defaults: {}, overrides: [{ matcher: { id: "byRegexp" as const, options: "/^CPU/i" }, properties: [{ id: "unit", value: "percent" }] }] };
    expect(resolveFieldOptions(ci, { name: "cpu_load", type: "number" }).unit).toBe("percent");
    const bad = { defaults: {}, overrides: [{ matcher: { id: "byRegexp" as const, options: "[" }, properties: [{ id: "unit", value: "percent" }] }] };
    expect(resolveFieldOptions(bad, { name: "cpu_load", type: "number" }).unit).toBeUndefined();
  });
});
