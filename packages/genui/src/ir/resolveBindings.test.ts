// resolveBindings + JSON-Pointer purity (genui-scope Testing plan: `resolveBindings` purity).
import { describe, it, expect } from "vitest";
import { resolvePointer, resolveValue, resolveBindings } from "./resolveBindings";

const data = {
  data: {
    A: { value: 42, rows: [{ t: 1, v: 10 }, { t: 2, v: 20 }] },
    B: { latest: "ok", nested: { deep: [1, 2, 3] } },
  },
};

describe("resolvePointer", () => {
  it("resolves object + array + nested paths", () => {
    expect(resolvePointer(data, "/data/A/value")).toBe(42);
    expect(resolvePointer(data, "/data/A/rows/1/v")).toBe(20);
    expect(resolvePointer(data, "/data/B/nested/deep/2")).toBe(3);
    expect(resolvePointer(data, "")).toBe(data);
  });
  it("returns undefined for missing/out-of-range/non-pointer", () => {
    expect(resolvePointer(data, "/data/Z/value")).toBeUndefined();
    expect(resolvePointer(data, "/data/A/rows/9")).toBeUndefined();
    expect(resolvePointer(data, "no-slash")).toBeUndefined();
    expect(resolvePointer(data, "/data/A/value/deeper")).toBeUndefined();
  });
  it("unescapes ~1 and ~0", () => {
    expect(resolvePointer({ "a/b": 1, "c~d": 2 }, "/a~1b")).toBe(1);
    expect(resolvePointer({ "a/b": 1, "c~d": 2 }, "/c~0d")).toBe(2);
  });
});

describe("resolveValue / resolveBindings", () => {
  it("swaps a $bind for the pointed value, deep", () => {
    expect(resolveValue({ $bind: "/data/A/value" }, data)).toBe(42);
    expect(resolveValue([{ $bind: "/data/A/value" }, "lit"], data)).toEqual([42, "lit"]);
    expect(resolveValue({ x: { $bind: "/data/B/latest" } }, data)).toEqual({ x: "ok" });
  });
  it("leaves literals intact; unresolvable → undefined", () => {
    expect(resolveValue("hello", data)).toBe("hello");
    expect(resolveValue({ $bind: "/nope" }, data)).toBeUndefined();
  });
  it("resolves a whole props bag", () => {
    const out = resolveBindings({ value: { $bind: "/data/A/value" }, label: "Temp" }, data);
    expect(out).toEqual({ value: 42, label: "Temp" });
  });
  it("is pure (does not mutate input)", () => {
    const snapshot = JSON.stringify(data);
    resolveBindings({ v: { $bind: "/data/A/value" } }, data);
    expect(JSON.stringify(data)).toBe(snapshot);
  });
});
