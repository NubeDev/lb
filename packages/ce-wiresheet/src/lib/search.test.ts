import { describe, expect, it } from "vitest";
import { buildSearchIndex, rankSearchHits } from "./search";
import { serializeFacet, FACET_PROP } from "./facet";
import type { Component, Property } from "./engine-types";

function prop(uid: number, componentUid: number): Property {
  return { uid, componentUid, category: 0, value: 0, statusFlags: 0 };
}
function comp(
  uid: number,
  name: string,
  parent: number,
  path: string,
  userProps: Record<string, number>,
  facet?: string,
  children?: Component[],
): Component {
  const properties: Record<string, Property> = {};
  for (const [n, p] of Object.entries(userProps)) properties[n] = prop(p, uid);
  if (facet != null) {
    properties[FACET_PROP] = { ...prop(900 + uid, uid), value: facet, systemRole: 2 };
  }
  return { uid, name, type: "math::add", path, parent, properties, children };
}

// root → pump (with facet: in1 labelled "Speed", aliased) ; sensor (no facet)
const facet = serializeFacet(
  new Map([[11, { label: "Speed", aliases: [{ code: 0, label: "off" }, { code: 1, label: "auto" }] }]]),
);
const tree: Component[] = [
  comp(0, "root", -1, "root", {}, undefined, [
    comp(1, "pump", 0, "root/pump", { in1: 11 }, facet),
    comp(2, "sensor", 0, "root/sensor", { in1: 21 }),
  ]),
];

describe("buildSearchIndex", () => {
  const idx = buildSearchIndex(tree, 0);

  it("skips root and strips the leading root/ from paths", () => {
    expect(idx.some((h) => h.compName === "root")).toBe(false);
    expect(idx.find((h) => h.compName === "pump")?.path).toBe("pump");
  });

  it("emits a component entry per component", () => {
    expect(idx.filter((h) => !h.propName).map((h) => h.compName).sort()).toEqual(["pump", "sensor"]);
  });

  it("emits a prop entry only for props with a facet label/aliases", () => {
    const props = idx.filter((h) => h.propName);
    expect(props).toHaveLength(1); // pump.in1 has a label; sensor.in1 has none
    expect(props[0]).toMatchObject({ compName: "pump", propName: "in1", label: "Speed" });
    expect(props[0].aliasText).toBe("off auto");
  });

  it("flags components in the current folder", () => {
    expect(buildSearchIndex(tree, 0).find((h) => h.compName === "pump")?.here).toBe(true);
    expect(buildSearchIndex(tree, 999).find((h) => h.compName === "pump")?.here).toBe(false);
  });
});

describe("rankSearchHits", () => {
  const idx = buildSearchIndex(tree, 0);

  it("with no query returns component rows only", () => {
    const r = rankSearchHits(idx, "");
    expect(r.every((h) => !h.propName)).toBe(true);
  });

  it("matches a prop by its facet label", () => {
    const r = rankSearchHits(idx, "speed");
    expect(r[0]).toMatchObject({ propName: "in1", label: "Speed" });
  });

  it("matches a prop by an alias label", () => {
    const r = rankSearchHits(idx, "auto");
    expect(r.some((h) => h.propName === "in1")).toBe(true);
  });

  it("matches a component by name", () => {
    const r = rankSearchHits(idx, "sensor");
    expect(r[0].compName).toBe("sensor");
  });
});
