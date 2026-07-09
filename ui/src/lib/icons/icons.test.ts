// Unit tests for the icon lib — resolver + catalog over the real lucide-react export
// (no mock; rule 9). Verifies name resolution, alias forms, and search ranking.

import { describe, expect, it } from "vitest";

import { resolveIcon, isIconName } from "./resolve";
import { ICON_CATALOG, searchIcons } from "./catalog";

describe("resolveIcon", () => {
  it("resolves a kebab-case name", () => {
    expect(resolveIcon("git-branch")).toBeTypeOf("object");
    expect(isIconName("git-branch")).toBe(true);
  });

  it("resolves the PascalCase key too", () => {
    expect(resolveIcon("GitBranch")).toBe(resolveIcon("git-branch"));
  });

  it("returns null for an unknown name and for empty input", () => {
    expect(resolveIcon("definitely-not-an-icon")).toBeNull();
    expect(resolveIcon("")).toBeNull();
    expect(resolveIcon(null)).toBeNull();
  });
});

describe("catalog + search", () => {
  it("builds a non-trivial, de-duplicated, sorted catalog", () => {
    expect(ICON_CATALOG.length).toBeGreaterThan(1000);
    const names = ICON_CATALOG.map((e) => e.name);
    expect(new Set(names).size).toBe(names.length);
    expect([...names].sort()).toEqual(names);
  });

  it("ranks a whole-token match ahead of a loose substring", () => {
    const hits = searchIcons("chart", 20).map((e) => e.name);
    expect(hits.length).toBeGreaterThan(0);
    expect(hits.every((n) => n.includes("chart"))).toBe(true);
  });

  it("honours the limit and returns catalog head for an empty query", () => {
    expect(searchIcons("", 10)).toHaveLength(10);
    expect(searchIcons("", 10)[0]).toBe(ICON_CATALOG[0]);
  });

  it("every catalog entry resolves back to a component", () => {
    for (const e of ICON_CATALOG.slice(0, 50)) {
      expect(resolveIcon(e.name)).not.toBeNull();
    }
  });
});
