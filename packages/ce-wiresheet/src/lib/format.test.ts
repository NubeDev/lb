import { describe, expect, it } from "vitest";
import { fmtValue, fmtValueFacet, inferDataType } from "./format";
import { DATATYPE_BOOL, DATATYPE_NUMBER, DATATYPE_STRING } from "./engine-types";

describe("fmtValue", () => {
  it("formats by type", () => {
    expect(fmtValue(undefined, DATATYPE_NUMBER)).toBe("—");
    expect(fmtValue(true, DATATYPE_BOOL)).toBe("true");
    expect(fmtValue(1, DATATYPE_BOOL)).toBe("true"); // numeric bool
    expect(fmtValue("hi", DATATYPE_STRING)).toBe("hi");
    expect(fmtValue(3, DATATYPE_NUMBER)).toBe("3"); // integer
    expect(fmtValue(3.14159, DATATYPE_NUMBER)).toBe("3.14"); // 2dp default
  });
});

describe("fmtValueFacet", () => {
  it("alias label wins", () => {
    const facet = { aliases: [{ code: 1, label: "auto" }] };
    expect(fmtValueFacet(1, DATATYPE_NUMBER, facet)).toBe("auto");
  });
  it("applies facet decimals + unit", () => {
    expect(fmtValueFacet(3.14159, DATATYPE_NUMBER, { decimals: 1, unit: "°C" })).toBe("3.1 °C");
  });
  it("clamps an out-of-range decimals instead of throwing (toFixed 0–100)", () => {
    expect(() => fmtValueFacet(1.23, DATATYPE_NUMBER, { decimals: 999 })).not.toThrow();
    expect(fmtValueFacet(1.23, DATATYPE_NUMBER, { decimals: -5 })).toBe("1"); // clamped to 0
  });
  it("ignores a NaN decimals", () => {
    expect(fmtValueFacet(3.14159, DATATYPE_NUMBER, { decimals: NaN })).toBe("3.14");
  });
});

describe("inferDataType", () => {
  it("maps runtime type to a dataType", () => {
    expect(inferDataType(true)).toBe(DATATYPE_BOOL);
    expect(inferDataType("x")).toBe(DATATYPE_STRING);
    expect(inferDataType(5)).toBe(DATATYPE_NUMBER);
  });
});

describe("datetime facet format", () => {
  it("renders an epoch as a local date/time string, not the raw number", () => {
    const out = fmtValueFacet(1765432123, DATATYPE_NUMBER, { format: "datetime" });
    expect(out).not.toBe("1765432123");
    expect(out).not.toBe("—");
    expect(out).toMatch(/\d/);
  });
  it("treats 0 / non-finite as empty", () => {
    expect(fmtValueFacet(0, DATATYPE_NUMBER, { format: "datetime" })).toBe("—");
  });
  it("auto-detects millisecond epochs", () => {
    const sec = fmtValueFacet(1765432123, DATATYPE_NUMBER, { format: "date" });
    const ms = fmtValueFacet(1765432123000, DATATYPE_NUMBER, { format: "date" });
    expect(ms).toBe(sec); // same instant, seconds vs millis
  });
});
