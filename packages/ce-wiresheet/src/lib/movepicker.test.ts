import { describe, expect, it } from "vitest";
import { moveCandidates, filterMoveCandidates } from "./movepicker";
import type { Component } from "./engine-types";

function comp(uid: number, name: string, parent: number, path: string): Component {
  return { uid, name, type: "math::add", path, parent, properties: {} };
}

// root → f1 → { c10 (moving), c11 } ; f1 also has child folder f2 → c20 ; root → other
const components: Component[] = [
  comp(0, "root", -1, "root"),
  comp(1, "f1", 0, "root/f1"),
  comp(10, "c10", 1, "root/f1/c10"), // the one being moved
  comp(11, "c11", 1, "root/f1/c11"),
  comp(2, "f2", 1, "root/f1/f2"),
  comp(20, "c20", 2, "root/f1/f2/c20"),
  comp(3, "other", 0, "root/other"),
];

describe("moveCandidates", () => {
  const cands = moveCandidates(components, [10]); // moving c10 (parent f1)

  it("excludes the moving component itself", () => {
    expect(cands.some((c) => c.uid === 10)).toBe(false);
  });

  it("tiers: up one level (root) → same level (c11,f2) → children (c20) → elsewhere", () => {
    const tierByName = Object.fromEntries(cands.map((c) => [c.name, c.tier]));
    expect(tierByName["root"]).toBe(0); // f1's parent
    expect(tierByName["c11"]).toBe(1); // sibling
    expect(tierByName["f2"]).toBe(1); // sibling
    expect(tierByName["c20"]).toBe(2); // deeper inside f1
    expect(tierByName["other"]).toBe(3);
  });

  it("sorts by tier then path", () => {
    expect(cands.map((c) => c.tier)).toEqual([...cands.map((c) => c.tier)].sort((a, b) => a - b));
  });

  it("excludes the moving component's own descendants (no cycles)", () => {
    // move f1 → its descendants c10/c11/f2/c20 must not be destinations
    const c = moveCandidates(components, [1]).map((x) => x.uid);
    expect(c).not.toContain(10);
    expect(c).not.toContain(20);
    expect(c).toContain(0); // root is still valid
  });
});

describe("filterMoveCandidates", () => {
  const cands = moveCandidates(components, [10]);
  it("matches name / kind / path, case-insensitive", () => {
    expect(filterMoveCandidates(cands, "OTHER").map((c) => c.name)).toEqual(["other"]);
    expect(filterMoveCandidates(cands, "f2").some((c) => c.name === "f2")).toBe(true);
  });
  it("returns all when the filter is blank", () => {
    expect(filterMoveCandidates(cands, "  ")).toHaveLength(cands.length);
  });
});
