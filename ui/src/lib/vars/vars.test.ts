// Exhaustive unit tests for the shared vars library (widget-config-vars scope, "Unit-test the lib
// exhaustively"). This is a FROZEN contract once extensions link it — every syntax, every format hint,
// multi-value, every built-in, nested tree, unknown-left-literal. Pure logic; no gateway.

import { describe, expect, it } from "vitest";

import {
  interpolate,
  interpolateArgs,
  resolveBuiltins,
  extractVarNames,
  extractVarNamesDeep,
  type VarScope,
} from "./index";

const scope = (
  values: VarScope["values"],
  builtins: VarScope["builtins"] = {},
): VarScope => ({ values, builtins });

describe("interpolate — the three reference syntaxes", () => {
  const s = scope({ host: "web01" });
  it("$var", () => expect(interpolate("cpu.$host", s)).toBe("cpu.web01"));
  it("${var}", () => expect(interpolate("cpu.${host}", s)).toBe("cpu.web01"));
  it("[[var]]", () => expect(interpolate("cpu.[[host]]", s)).toBe("cpu.web01"));
  it("mixed in one string", () =>
    expect(interpolate("$host/${host}/[[host]]", s)).toBe("web01/web01/web01"));
});

describe("interpolate — unknown variable is left literal (Grafana behavior, never throws)", () => {
  it("leaves $unknown / ${unknown} / [[unknown]] untouched", () => {
    const s = scope({});
    expect(interpolate("$nope", s)).toBe("$nope");
    expect(interpolate("${nope}", s)).toBe("${nope}");
    expect(interpolate("[[nope]]", s)).toBe("[[nope]]");
  });
});

describe("interpolate — format hints (multi-value aware)", () => {
  const single = scope({ host: "web01" });
  const multi = scope({ host: ["web01", "web02"] });

  it("default: single is itself, multi joins with commas", () => {
    expect(interpolate("${host}", single)).toBe("web01");
    expect(interpolate("${host}", multi)).toBe("web01,web02");
  });
  it("csv", () => expect(interpolate("${host:csv}", multi)).toBe("web01,web02"));
  it("pipe", () => expect(interpolate("${host:pipe}", multi)).toBe("web01|web02"));
  it("singlequote", () =>
    expect(interpolate("${host:singlequote}", multi)).toBe("'web01','web02'"));
  it("doublequote", () =>
    expect(interpolate("${host:doublequote}", multi)).toBe('"web01","web02"'));
  it("json (single → quoted scalar, multi → array)", () => {
    expect(interpolate("${host:json}", single)).toBe('"web01"');
    expect(interpolate("${host:json}", multi)).toBe('["web01","web02"]');
  });
  it("raw", () => expect(interpolate("${host:raw}", multi)).toBe("web01,web02"));
  it("the hint also works in [[var:fmt]] form", () =>
    expect(interpolate("[[host:pipe]]", multi)).toBe("web01|web02"));
});

describe("resolveBuiltins — every built-in", () => {
  const b = resolveBuiltins({
    timeRange: { fromMs: 1000, toMs: 61000 },
    identity: { login: "bob", email: "bob@x.io" },
    dashboardId: "ops",
    workspace: "acme",
    interval: "5m",
    value: "42",
  });
  const s = scope({}, b);

  it("$__from / $__to (epoch ms)", () => {
    expect(interpolate("$__from", s)).toBe("1000");
    expect(interpolate("$__to", s)).toBe("61000");
  });
  it("$__range / $__range_s / $__range_ms", () => {
    expect(interpolate("$__range", s)).toBe("60s");
    expect(interpolate("$__range_s", s)).toBe("60");
    expect(interpolate("$__range_ms", s)).toBe("60000");
  });
  it("$__interval / $__interval_ms", () => {
    expect(interpolate("$__interval", s)).toBe("5m");
    expect(interpolate("$__interval_ms", s)).toBe("300000");
  });
  it("${__user.login} / ${__user.email}", () => {
    expect(interpolate("${__user.login}", s)).toBe("bob");
    expect(interpolate("${__user.email}", s)).toBe("bob@x.io");
  });
  it("${__dashboard} / ${__workspace} / ${__value}", () => {
    expect(interpolate("${__dashboard}", s)).toBe("ops");
    expect(interpolate("${__workspace}", s)).toBe("acme");
    expect(interpolate("${__value}", s)).toBe("42");
  });
  it("a missing input yields no key (the reference stays literal, not a fake empty)", () => {
    const bare = resolveBuiltins({});
    expect(interpolate("${__workspace}", scope({}, bare))).toBe("${__workspace}");
  });
});

describe("interpolateArgs — deep, type-preserving substitution over a JSON tree", () => {
  const s = scope({ host: "web01", hosts: ["web01", "web02"] }, resolveBuiltins({ workspace: "acme" }));

  it("substitutes embedded references in a string leaf (formats applied)", () => {
    expect(interpolateArgs({ series: "cpu.${host}" }, s)).toEqual({ series: "cpu.web01" });
  });
  it("a SOLE multi-value reference becomes a real ARRAY (type-preserving, for a JSON IN sink)", () => {
    expect(interpolateArgs({ hosts: "${hosts}" }, s)).toEqual({ hosts: ["web01", "web02"] });
  });
  it("a SOLE single reference keeps the raw value", () => {
    expect(interpolateArgs({ ws: "${__workspace}" }, s)).toEqual({ ws: "acme" });
  });
  it("non-string leaves pass through untouched (no coercion)", () => {
    expect(interpolateArgs({ n: 5, b: true, z: null }, s)).toEqual({ n: 5, b: true, z: null });
  });
  it("recurses arrays + nested objects", () => {
    const tree = { a: ["cpu.$host", { deep: "[[host]]" }], b: { c: "$__workspace" } };
    expect(interpolateArgs(tree, s)).toEqual({
      a: ["cpu.web01", { deep: "web01" }],
      b: { c: "acme" },
    });
  });
  it("unknown SOLE reference is left literal", () => {
    expect(interpolateArgs({ x: "${nope}" }, s)).toEqual({ x: "${nope}" });
  });

  // The argsTemplate generalization: `{{value}}` / `${__value}` fill the runtime interaction value,
  // type-preserving — the cases the shipped control argsTemplate test asserts must stay green.
  it("fills {{value}} with the runtime value, preserving type (bool/number)", () => {
    expect(interpolateArgs({ topic: "acme/x", payload: "{{value}}" }, s, true)).toEqual({
      topic: "acme/x",
      payload: true,
    });
    expect(interpolateArgs({ level: "{{value}}" }, s, 42)).toEqual({ level: 42 });
  });
  it("${__value} is the built-in alias of {{value}}", () => {
    expect(interpolateArgs({ v: "${__value}" }, s, 7)).toEqual({ v: 7 });
  });
});

describe("extractVarNames — refresh deps + the deny-set", () => {
  it("collects all three syntaxes (built-ins included), first-seen order, deduped", () => {
    expect(extractVarNames("$host/${region}/[[host]]/$__from")).toEqual([
      "host",
      "region",
      "__from",
    ]);
  });
  it("walks a JSON tree (extractVarNamesDeep)", () => {
    const names = extractVarNamesDeep({ s: "cpu.$host", n: 1, a: ["${region}"] });
    expect(names.sort()).toEqual(["host", "region"]);
  });
});
