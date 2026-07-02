import { describe, expect, it } from "vitest";
import { buildConnectGroups, filterConnectGroups, takenInputUids, connectTier } from "./connect";
import {
  CATEGORY_INPUT,
  CATEGORY_OUTPUT,
  type Component,
  type Edge,
  type Property,
  type PropertyCategory,
  type PropertySystemRole,
} from "./engine-types";

function p(
  uid: number,
  componentUid: number,
  category: PropertyCategory,
  systemRole: PropertySystemRole = 0,
): Property {
  return { uid, componentUid, category, value: 0, statusFlags: 0, systemRole };
}
function comp(
  uid: number,
  name: string,
  parent: number,
  path: string,
  props: Record<string, Property>,
): Component {
  return { uid, name, type: "math::add", path, parent, properties: props };
}

// source = c10 (output), inside folder f1. Candidates with INPUT props:
//   f1   (parent)       in: i_f1
//   c11  (sibling)      in: i_c11
//   c12  (child of c10) in: i_c12
//   c20  (elsewhere)    in: i_c20
const components: Component[] = [
  comp(1, "f1", 0, "root/f1", { x: p(901, 1, CATEGORY_INPUT) }),
  comp(11, "c11", 1, "root/f1/c11", { in: p(111, 11, CATEGORY_INPUT) }),
  comp(12, "c12", 10, "root/f1/c10/c12", { in: p(121, 12, CATEGORY_INPUT) }),
  comp(20, "c20", 2, "root/other/c20", { in: p(201, 20, CATEGORY_INPUT) }),
  comp(10, "c10", 1, "root/f1/c10", { out: p(101, 10, CATEGORY_OUTPUT) }),
];

const opts = {
  sourceComponentUid: 10,
  sourceParent: 1 as number | undefined,
  wantCategory: CATEGORY_INPUT as PropertyCategory,
  taken: new Set<number>(),
};

describe("takenInputUids", () => {
  it("collects target prop uids of existing edges", () => {
    const edges: Edge[] = [
      { uid: 1, sourceUid: 1, sourceProperty: "o", targetUid: 2, targetProperty: "i", targetPropertyUid: 111 },
      { uid: 2, sourceUid: 3, sourceProperty: "o", targetUid: 4, targetProperty: "i" }, // no uid → skipped
    ];
    expect([...takenInputUids(edges)]).toEqual([111]);
  });
});

describe("buildConnectGroups", () => {
  it("tiers parent → same level → child → elsewhere", () => {
    const groups = buildConnectGroups(components, opts);
    expect(groups.map((g) => g.componentName)).toEqual(["f1", "c11", "c12", "c20"]);
    expect(groups.map(connectTier)).toEqual([0, 1, 2, 3]);
  });

  it("excludes the source itself and components with no matching props", () => {
    const groups = buildConnectGroups(components, opts);
    expect(groups.some((g) => g.componentUid === 10)).toBe(false);
  });

  it("hides props that are already taken", () => {
    const groups = buildConnectGroups(components, { ...opts, taken: new Set([111]) });
    expect(groups.some((g) => g.componentUid === 11)).toBe(false); // its only input is taken
  });

  it("skips system-role props", () => {
    const withSys = [comp(30, "c30", 1, "root/f1/c30", { __s: p(301, 30, CATEGORY_INPUT, 2) })];
    expect(buildConnectGroups(withSys, opts)).toHaveLength(0);
  });
});

describe("filterConnectGroups", () => {
  const groups = buildConnectGroups(components, opts);

  it("matches by component name", () => {
    expect(filterConnectGroups(groups, "c20").map((g) => g.componentName)).toEqual(["c20"]);
  });

  it("path scope 'f1/c10/c' restricts to that folder and deeper", () => {
    const r = filterConnectGroups(groups, "f1/c10/c");
    expect(r.map((g) => g.componentName)).toEqual(["c12"]); // c12 is under f1/c10
  });

  it("a pure folder scope (trailing slash) keeps every group whose path is in that folder", () => {
    const r = filterConnectGroups(groups, "f1/");
    // f1 itself + everything under it; c20 (root/other/...) is excluded.
    expect(r.map((g) => g.componentName).sort()).toEqual(["c11", "c12", "f1"]);
  });
});
