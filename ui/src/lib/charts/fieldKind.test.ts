import { describe, expect, it } from "vitest";

import { defaultXField, inferFields, numericFields } from "./fieldKind";

describe("inferFields", () => {
  it("types temporal, numeric and categorical columns from sampled values", () => {
    const rows = [
      { t: "2026-01-01T00:00:00Z", host: "a", cpu: 12 },
      { t: "2026-01-01T00:01:00Z", host: "b", cpu: 34 },
    ];
    const fields = inferFields(rows);
    expect(fields).toEqual([
      { name: "t", kind: "time" },
      { name: "host", kind: "category" },
      { name: "cpu", kind: "number" },
    ]);
  });

  it("reads a large integer in the epoch band as time, small integers as number", () => {
    const rows = [{ ts: 1_700_000_000, n: 3 }];
    expect(inferFields(rows)).toEqual([
      { name: "ts", kind: "time" },
      { name: "n", kind: "number" },
    ]);
  });

  it("treats numeric strings as numbers (stringified SurrealDB frames)", () => {
    const rows = [{ v: "1.5" }, { v: "2.0" }];
    expect(inferFields(rows)[0]).toEqual({ name: "v", kind: "number" });
  });

  it("falls back to category for a mixed column", () => {
    const rows = [{ v: 1 }, { v: "hello" }];
    expect(inferFields(rows)[0].kind).toBe("category");
  });

  it("preserves first-seen column order across ragged rows", () => {
    const rows = [{ a: 1 }, { b: 2, a: 3 }];
    expect(inferFields(rows).map((f) => f.name)).toEqual(["a", "b"]);
  });
});

describe("numericFields / defaultXField", () => {
  const fields = inferFields([{ t: "2026-01-01T00:00:00Z", host: "a", cpu: 1, mem: 2 }]);

  it("lists numeric columns as y candidates", () => {
    expect(numericFields(fields)).toEqual(["cpu", "mem"]);
  });

  it("prefers a temporal x, else categorical, else first column", () => {
    expect(defaultXField(fields)).toBe("t");
    expect(defaultXField(inferFields([{ host: "a", cpu: 1 }]))).toBe("host");
    expect(defaultXField(inferFields([{ cpu: 1, mem: 2 }]))).toBe("cpu");
  });
});
