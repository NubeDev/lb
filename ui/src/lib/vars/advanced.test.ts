// Unit tests for the advanced-variables lib additions (advanced-variables scope, "Unit (`vars/*.test.ts`)").
// Pure logic: label/value split, regex extraction, sort, dependency ordering + honest cycle, new format
// hints. No gateway. The FROZEN resolved `VarScope` shape is unchanged — these are all definition-side.

import { describe, expect, it } from "vitest";

import {
  parseCustomOptions,
  applyRegex,
  sortOptions,
  orderVariables,
  dependentsOf,
  VarCycleError,
  formatValue,
  type Variable,
  type VariableOption,
} from "./index";

describe("parseCustomOptions — label : value split", () => {
  it("a bare string is text=value", () =>
    expect(parseCustomOptions(["web01"])).toEqual([{ text: "web01", value: "web01" }]));
  it("splits `text : value` on the first space-colon-space", () =>
    expect(parseCustomOptions(["West : WST"])).toEqual([{ text: "West", value: "WST" }]));
  it("keeps an escaped colon literal", () =>
    expect(parseCustomOptions(["ratio\\:1 : x"])).toEqual([{ text: "ratio:1", value: "x" }]));
  it("no ` : ` → whole string is the value (a URL with `://` is not split)", () =>
    expect(parseCustomOptions(["http://x"])).toEqual([{ text: "http://x", value: "http://x" }]));
});

describe("applyRegex — filter + named-capture split", () => {
  const opts: VariableOption[] = [
    { text: "West (WST)", value: "West (WST)" },
    { text: "East (EST)", value: "East (EST)" },
    { text: "skip me", value: "skip me" },
  ];
  it("named (?<text>)/(?<value>) groups split text≠value; non-matches dropped", () => {
    const out = applyRegex(opts, "(?<text>.+) \\((?<value>[A-Z]+)\\)", "value");
    expect(out).toEqual([
      { text: "West", value: "WST" },
      { text: "East", value: "EST" },
    ]);
  });
  it("applyTo:text matches against the display text", () => {
    const out = applyRegex(
      [{ text: "prod-web", value: "v1" }, { text: "dev-web", value: "v2" }],
      "^prod-",
      "text",
    );
    expect(out).toEqual([{ text: "prod-web", value: "v1" }]);
  });
  it("an invalid regex fails honestly — returns options unchanged, drops nothing", () =>
    expect(applyRegex(opts, "(unclosed", "value")).toEqual(opts));
  it("no regex → unchanged", () => expect(applyRegex(opts, undefined)).toEqual(opts));
});

describe("sortOptions", () => {
  const o = (t: string): VariableOption => ({ text: t, value: t });
  it("none keeps insertion order", () =>
    expect(sortOptions([o("b"), o("a")], "none")).toEqual([o("b"), o("a")]));
  it("alphaAsc / alphaDesc", () => {
    expect(sortOptions([o("b"), o("a"), o("c")], "alphaAsc").map((x) => x.text)).toEqual(["a", "b", "c"]);
    expect(sortOptions([o("b"), o("a"), o("c")], "alphaDesc").map((x) => x.text)).toEqual(["c", "b", "a"]);
  });
  it("numAsc sorts numerically not lexically", () =>
    expect(sortOptions([o("10"), o("2"), o("1")], "numAsc").map((x) => x.text)).toEqual(["1", "2", "10"]));
  it("alphaCiAsc is case-insensitive", () =>
    expect(sortOptions([o("B"), o("a"), o("C")], "alphaCiAsc").map((x) => x.text)).toEqual(["a", "B", "C"]));
});

describe("orderVariables — chained resolution ordering", () => {
  const v = (name: string, sql?: string): Variable => ({
    name,
    type: "query",
    query: sql ? { tool: "store.query", args: { sql } } : undefined,
  });

  it("orders a $region→$host chain region-first", () => {
    const region = v("region", "SELECT name FROM region");
    const host = v("host", "SELECT h FROM node WHERE region = '$region'");
    const ordered = orderVariables([host, region]).map((x) => x.name);
    expect(ordered.indexOf("region")).toBeLessThan(ordered.indexOf("host"));
  });

  it("independent variables keep insertion order", () =>
    expect(orderVariables([v("a"), v("b"), v("c")]).map((x) => x.name)).toEqual(["a", "b", "c"]));

  it("a cycle fails honestly (throws VarCycleError, never hangs)", () => {
    const a = v("a", "x = '$b'");
    const b = v("b", "y = '$a'");
    expect(() => orderVariables([a, b])).toThrow(VarCycleError);
  });

  it("dependentsOf returns the transitive downstream set", () => {
    const region = v("region");
    const host = v("host", "region = '$region'");
    const pod = v("pod", "host = '$host'");
    expect(dependentsOf([region, host, pod], "region")).toEqual(new Set(["host", "pod"]));
  });
});

describe("formatValue — new advanced hints", () => {
  it("regex escapes + alternates a multi-value", () =>
    expect(formatValue(["web01", "web.02"], "regex")).toBe("(web01|web\\.02)"));
  it("regex escapes a single value", () => expect(formatValue("a.b", "regex")).toBe("a\\.b"));
  it("glob alternates", () => expect(formatValue(["a", "b"], "glob")).toBe("{a,b}"));
  it("percentencode", () => expect(formatValue("a b/c", "percentencode")).toBe("a%20b%2Fc"));
  it("sqlstring doubles single quotes", () =>
    expect(formatValue(["O'Neil", "x"], "sqlstring")).toBe("'O''Neil','x'"));
});
