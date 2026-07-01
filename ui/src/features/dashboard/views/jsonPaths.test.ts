// Unit tests for the JSON-path utilities + path-aware flow extraction (flow-dashboard-binding-ux-scope:
// "parse out the JSON"). Proves the visual builder's selection resolves the right leaf for objects,
// arrays, and nested shapes — agnostic to the node type a developer ships.

import { describe, it, expect } from "vitest";

import {
  asPath,
  childrenOf,
  kindOf,
  pathLabel,
  previewOf,
  valueAtPath,
} from "./jsonPaths";
import { extractFlowValue } from "./useFlowNodeValue";

describe("json path utilities", () => {
  const root = {
    payload: { cron_ts: 1782861480, band: [3.5, 4.5], mode: "eco" },
    topic: "kfc.temp",
    items: [{ name: "a" }, { name: "b" }],
  };

  it("kindOf classifies every JSON shape", () => {
    expect(kindOf({})).toBe("object");
    expect(kindOf([])).toBe("array");
    expect(kindOf("x")).toBe("string");
    expect(kindOf(1)).toBe("number");
    expect(kindOf(true)).toBe("boolean");
    expect(kindOf(null)).toBe("null");
    expect(kindOf(undefined)).toBe("null");
  });

  it("walks a path into objects, arrays, and nested fields", () => {
    expect(valueAtPath(root, ["payload"])).toEqual(root.payload);
    expect(valueAtPath(root, ["payload", "cron_ts"])).toBe(1782861480);
    expect(valueAtPath(root, ["payload", "band", 1])).toBe(4.5);
    expect(valueAtPath(root, ["items", 0, "name"])).toBe("a");
    expect(valueAtPath(root, [])).toBe(root); // whole value
  });

  it("returns undefined for a missing segment (honest 'not there')", () => {
    expect(valueAtPath(root, ["nope"])).toBeUndefined();
    expect(valueAtPath(root, ["payload", "nope"])).toBeUndefined();
    expect(valueAtPath(root, ["items", 9, "name"])).toBeUndefined();
  });

  it("enumerates a container's directly-addressable children", () => {
    const objKids = childrenOf(root.payload).map((c) => c.label);
    expect(objKids).toEqual(["cron_ts", "band", "mode"]);
    const arrKids = childrenOf(root.items).map((c) => c.label);
    expect(arrKids).toEqual(["[0]", "[1]"]);
    expect(childrenOf(42)).toEqual([]); // a scalar leaf has no children
  });

  it("renders a human path label + value preview", () => {
    expect(pathLabel([])).toBe("(whole value)");
    expect(pathLabel(["payload", "cron_ts"])).toBe("payload.cron_ts");
    expect(pathLabel(["items", 0, "name"])).toBe("items[0].name");
    expect(previewOf({ a: 1, b: 2 })).toBe("{2}");
    expect(previewOf([1, 2, 3])).toBe("[3]");
    expect(previewOf("eco")).toBe('"eco"');
    expect(previewOf(55)).toBe("55");
  });

  it("asPath tolerates absent/garbage stored paths", () => {
    expect(asPath(undefined)).toEqual([]);
    expect(asPath(null)).toEqual([]);
    expect(asPath(["payload", 0])).toEqual(["payload", 0]);
    expect(asPath([{ bad: 1 }])).toEqual([]); // non-segment entries dropped
  });
});

describe("path-aware flow value extraction", () => {
  // A node_state entry whose recorded value is a structured envelope.
  const entry = {
    node: "start",
    value: { payload: { cron_ts: 1782861480, band: [3.5, 4.5] }, topic: "t" },
    rev: 55,
  };

  it("a picked path extracts exactly that leaf (the 'parse out the JSON' payoff)", () => {
    expect(extractFlowValue(entry, "output", "payload", ["payload", "cron_ts"])).toBe(1782861480);
    expect(extractFlowValue(entry, "output", "payload", ["payload", "band", 0])).toBe(3.5);
    expect(extractFlowValue(entry, "output", "payload", ["topic"])).toBe("t");
  });

  it("no path (undefined) → the port's value (back-compat with the simple binding)", () => {
    expect(extractFlowValue(entry, "output", "payload")).toEqual({ cron_ts: 1782861480, band: [3.5, 4.5] });
  });

  it("an EXPLICIT empty path ([] = '(whole value)') reads the WHOLE value, not the port — so the path", () => {
    // picker preview and the widget agree: selecting "(whole value)" shows the whole envelope.
    expect(extractFlowValue(entry, "output", "payload", [])).toEqual(entry.value);
    // while a port-field path reads just that field.
    expect(extractFlowValue(entry, "output", "payload", ["payload"])).toEqual(entry.value.payload);
  });

  it("a missing picked path resolves to null, never a stale value", () => {
    expect(extractFlowValue(entry, "output", "payload", ["payload", "gone"])).toBeNull();
  });
});
