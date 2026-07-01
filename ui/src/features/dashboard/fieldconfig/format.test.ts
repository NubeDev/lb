// The user-prefs format bridge (viz field-config scope, Testing plan). Asserts: the bridge is the one
// formatter (unit/decimals applied here, not in a renderer); the sequencing FALLBACK is honest
// (canonical value + static unit, NO conversion until lb-prefs ships) and is flagged `viaPrefs:false`
// so the swap point is testable; an unmapped unit degrades to a passthrough number + suffix, never
// throwing; and NO formatted string is anything but computed-at-render (there is no stored string —
// `formatValue` is pure over the canonical value).

import { describe, it, expect } from "vitest";

import { formatValue } from "./format";
import { resolveUnit } from "./units";

describe("format bridge (fallback until lb-prefs)", () => {
  it("applies decimals through the bridge (not a renderer toFixed)", () => {
    expect(formatValue(12.3456, { decimals: 1 }).number).toBe("12.3");
    expect(formatValue(12.3456, { decimals: 0 }).number).toBe("12");
  });

  it("renders a dimensionful unit as canonical value + static label (no conversion in the fallback)", () => {
    const f = formatValue(12, { unit: "velocityms", decimals: 1 });
    // FALLBACK: the canonical 12 m/s, NOT converted to km/h — that arrives when lb-prefs ships.
    expect(f.text).toBe("12.0 m/s");
    expect(f.viaPrefs).toBe(false);
  });

  it("a percent unit is a localized number + literal sign (passthrough)", () => {
    expect(formatValue(42.5, { unit: "percent", decimals: 1 }).text).toBe("42.5%");
  });

  it("an UNMAPPED unit degrades to a passthrough number + raw suffix, never throws", () => {
    const f = formatValue(7, { unit: "furlongs", decimals: 0 });
    expect(() => formatValue(7, { unit: "furlongs" })).not.toThrow();
    expect(f.text).toBe("7 furlongs");
  });

  it("a non-numeric value is shown as text (no number math, no fabricated 0)", () => {
    expect(formatValue("offline", { unit: "celsius" }).text).toBe("offline");
    expect(formatValue(null, {}).text).toBe("");
  });

  it("the call site is format.*-shaped: every result carries viaPrefs so the swap point is testable", () => {
    // Until lb-prefs ships, EVERY path is the fallback (viaPrefs:false). When it lands, format.ts flips
    // these to true with no caller/schema change — this assertion is the guardrail for that swap.
    for (const unit of ["celsius", "percent", "short", undefined]) {
      expect(formatValue(1, { unit }).viaPrefs).toBe(false);
    }
  });
});

describe("unit mapping table", () => {
  it("maps known Grafana ids to a dimension or an explicit passthrough", () => {
    expect(resolveUnit("celsius")).toMatchObject({ kind: "quantity", dimension: "temperature" });
    expect(resolveUnit("percent").kind).toBe("number");
    expect(resolveUnit("short").kind).toBe("none");
    expect(resolveUnit("currencyUSD").kind).toBe("number");
  });

  it("custom:<suffix> and unknown ids are passthrough, never cross-dimension", () => {
    expect(resolveUnit("custom:widgets")).toMatchObject({ kind: "number", suffix: " widgets" });
    expect(resolveUnit("totallyunknown").kind).toBe("number");
  });

  it("datetime units carry the epoch dateUnit — flow-seconds is `s`, the rest default `ms`", () => {
    // The load-bearing distinction (flow-ts-display scope): the flow clock is epoch SECONDS, everything
    // else epoch ms. Declared on the mapping, NEVER guessed from the integer magnitude.
    expect(resolveUnit("time:flow-seconds")).toMatchObject({ kind: "datetime", dateUnit: "s" });
    expect(resolveUnit("dateTimeAsIso")).toMatchObject({ kind: "datetime", dateUnit: "ms" });
    expect(resolveUnit("time:YYYY-MM-DD")).toMatchObject({ kind: "datetime", dateUnit: "ms" });
  });
});

describe("datetime fallback honors the epoch unit (seconds vs ms)", () => {
  const INSTANT_MS = 1782864300000; // 2026-07-01T00:05:00Z
  it("a flow epoch-SECONDS value renders the right YEAR in the fallback (not 1970)", () => {
    // Before the async prefs render settles, the sync fallback still shows a sane date because
    // `dateUnit:"s"` triggers the ×1000. A magnitude-guessing formatter would render 1970 here.
    const f = formatValue(1782864300, { unit: "time:flow-seconds" });
    expect(f.text).toBe(new Date(INSTANT_MS).toISOString());
    expect(f.text.startsWith("2026-")).toBe(true);
  });

  it("an epoch-MS datetime value is unchanged (no spurious ×1000)", () => {
    const f = formatValue(INSTANT_MS, { unit: "dateTimeAsIso" });
    expect(f.text).toBe(new Date(INSTANT_MS).toISOString());
  });
});
