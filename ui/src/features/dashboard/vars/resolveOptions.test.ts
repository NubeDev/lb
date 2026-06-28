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

describe("rowsToOptions — shapes our read tools return", () => {
  it("{ rows: [{name}] } (store.query / series.find)", () => {
    expect(rowsToOptions({ rows: [{ name: "web01" }, { name: "web02" }] })).toEqual([
      { value: "web01", label: "web01" },
      { value: "web02", label: "web02" },
    ]);
  });
  it("a bare array of strings", () => {
    expect(rowsToOptions(["a", "b"])).toEqual([
      { value: "a", label: "a" },
      { value: "b", label: "b" },
    ]);
  });
  it("falls back to the first scalar column", () => {
    expect(rowsToOptions({ rows: [{ ts: 1, host: "web01" }] })).toEqual([
      { value: "1", label: "1" },
    ]);
  });
  it("dedupes + drops empties, never throws on a bad shape", () => {
    expect(rowsToOptions({ rows: ["a", "a", "", { name: "a" }] })).toEqual([{ value: "a", label: "a" }]);
    expect(rowsToOptions(null)).toEqual([]);
    expect(rowsToOptions(42)).toEqual([]);
  });
});
