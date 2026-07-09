// Unit tests for the variable-options shaping (widget-config-vars Slice 2) — pure, no bridge. Proves
// the one-model resolver: static lists for custom/interval, and tool rows → options for query/source
// across the shapes our read tools return.

import { describe, expect, it } from "vitest";

import { staticOptions, isQueryVariable, rowsToOptions } from "./resolveOptions";
import type { Variable } from "@/lib/vars";

const v = (over: Partial<Variable>): Variable => ({ name: "x", type: "custom", ...over });

describe("staticOptions", () => {
  it("custom → its list", () =>
    expect(staticOptions(v({ type: "custom", custom: ["prod", "staging"] }))).toEqual([
      { value: "prod", label: "prod" },
      { value: "staging", label: "staging" },
    ]));
  it("interval → its list", () =>
    expect(staticOptions(v({ type: "interval", interval: ["1m", "5m"] }))).toEqual([
      { value: "1m", label: "1m" },
      { value: "5m", label: "5m" },
    ]));
  it("query/text → [] (no static list)", () => {
    expect(staticOptions(v({ type: "query" }))).toEqual([]);
    expect(staticOptions(v({ type: "text" }))).toEqual([]);
  });
});

describe("isQueryVariable", () => {
  it("true only for query/source with a tool", () => {
    expect(isQueryVariable(v({ type: "query", query: { tool: "store.query" } }))).toBe(true);
    expect(isQueryVariable(v({ type: "source", query: { tool: "series.find" } }))).toBe(true);
    expect(isQueryVariable(v({ type: "query" }))).toBe(false); // no tool
    expect(isQueryVariable(v({ type: "custom" }))).toBe(false);
  });
});

describe("rowsToOptions — shapes our read tools return ({text,value})", () => {
  it("{ rows: [{name}] } (store.query / series.find)", () => {
    expect(rowsToOptions({ rows: [{ name: "web01" }, { name: "web02" }] })).toEqual([
      { text: "web01", value: "web01" },
      { text: "web02", value: "web02" },
    ]);
  });
  it("a bare array of strings", () => {
    expect(rowsToOptions(["a", "b"])).toEqual([
      { text: "a", value: "a" },
      { text: "b", value: "b" },
    ]);
  });
  it("extracts the `series` array (series.find / series.list shape)", () => {
    expect(rowsToOptions({ series: ["series:a", "series:b"] })).toEqual([
      { text: "series:a", value: "series:a" },
      { text: "series:b", value: "series:b" },
    ]);
  });
  it("honors the __text/__value convention (text ≠ value)", () => {
    expect(rowsToOptions({ rows: [{ __text: "West", __value: "WST" }] })).toEqual([
      { text: "West", value: "WST" },
    ]);
  });
  it("falls back to the first scalar column for the value", () => {
    expect(rowsToOptions({ rows: [{ ts: 1, host: "web01" }] })).toEqual([{ text: "1", value: "1" }]);
  });
  it("dedupes + drops empties, never throws on a bad shape", () => {
    expect(rowsToOptions({ rows: ["a", "a", "", { name: "a" }] })).toEqual([{ text: "a", value: "a" }]);
    expect(rowsToOptions(null)).toEqual([]);
    expect(rowsToOptions(42)).toEqual([]);
  });
});
