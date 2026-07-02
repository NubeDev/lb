import { describe, expect, it } from "vitest";
import { planPaste } from "./paste";
import { serializeFacet, parseFacet, FACET_PROP } from "./facet";
import type { Component, Property } from "./engine-types";

function prop(uid: number, componentUid: number): Property {
  return { uid, componentUid, category: 0, value: 0, statusFlags: 0 };
}
function comp(
  uid: number,
  parent: number,
  pos: { x: number; y: number },
  facet?: string,
  children?: Component[],
): Component {
  const properties: Record<string, Property> = {};
  if (facet != null) properties[FACET_PROP] = { ...prop(900 + uid, uid), value: facet, systemRole: 2 };
  return {
    uid,
    name: `c${uid}`,
    type: "math::add",
    path: `root/c${uid}`,
    parent,
    metadata: { position: pos },
    properties,
    children,
  };
}

describe("planPaste", () => {
  it("centres the VISUAL bounding box (positions + node size) on the cursor", () => {
    const clones = [comp(1, 0, { x: 0, y: 0 }), comp(2, 0, { x: 100, y: 0 })];
    const { updates, newUids } = planPaste(clones, 0, { x: 250, y: 250 });
    // bbox incl. node size: x 0..(100+NODE_W=320) → centre 160; y 0..(title+0 rows=44)
    // → centre 22. cursor (250,250) ⇒ dx=+90, dy=+228. The row ends up centred under
    // the pointer (not the middle node's top-left corner, the pre-fix behaviour).
    expect(updates.find((u) => u.uid === 1)?.position).toEqual({ x: 90, y: 228 });
    expect(updates.find((u) => u.uid === 2)?.position).toEqual({ x: 190, y: 228 });
    expect(newUids).toEqual([1, 2]);
  });

  it("preserves the copy-time grab offset when copyCursor is given", () => {
    // Node at (100,100); copy mouse grabbed it at (120,110). Paste mouse at (500,500)
    // → the node keeps the same offset from the mouse: top-left = paste + (pos-copy)
    // = (500-20, 500-10) = (480,490).
    const clones = [comp(1, 0, { x: 100, y: 100 })];
    const { updates } = planPaste(clones, 0, { x: 500, y: 500 }, { copyCursor: { x: 120, y: 110 } });
    expect(updates.find((u) => u.uid === 1)?.position).toEqual({ x: 480, y: 490 });
  });

  it("clamps the grab offset when the copy mouse was far from the selection", () => {
    // Copy mouse 5000px left of the node → without the clamp the paste lands 5000px
    // right of the cursor; PASTE_CLAMP (400) caps the cluster's top-left near it.
    const clones = [comp(1, 0, { x: 0, y: 0 })];
    const { updates } = planPaste(clones, 0, { x: 1000, y: 1000 }, { copyCursor: { x: -5000, y: 0 } });
    expect(updates.find((u) => u.uid === 1)?.position).toEqual({ x: 1400, y: 1000 });
  });

  it("repositions only top-level clones; descendants are flattened but not moved/selected", () => {
    const folder = comp(1, 0, { x: 0, y: 0 }, undefined, [comp(2, 1, { x: 10, y: 10 })]);
    const { updates, newUids } = planPaste([folder], 0, { x: 0, y: 0 });
    expect(newUids).toEqual([1]); // only the folder (parent === dest)
    expect(updates.find((u) => u.uid === 2)?.position).toBeUndefined();
  });

  it("remaps copied __facets uid references using the uidMap", () => {
    const facet = serializeFacet(
      new Map([[200, { expose: "input", childComponent: 50, facetProp: 60 }]]),
    );
    const folder = comp(1, 0, { x: 0, y: 0 }, facet);
    const { updates } = planPaste([folder], 0, { x: 0, y: 0 }, {
      uidMap: { components: { 50: 5050 }, properties: { 200: 2002, 60: 6060 } },
    });
    const remapped = parseFacet(updates.find((u) => u.uid === 1)!.properties![FACET_PROP].value);
    const rec = remapped.get(2002);
    expect(rec?.childComponent).toBe(5050);
    expect(rec?.facetProp).toBe(6060);
  });

  it("remaps a CHAINED port (groups-of-groups) — childComponent is the inner folder", () => {
    // Outer folder 1 chains inner folder 9's port (deep prop 200). On copy the
    // key (200), the inner-folder ref (9), and the facetProp (88) all remap.
    const facet = serializeFacet(
      new Map([[200, { expose: "output", childComponent: 9, facetProp: 88, chain: true }]]),
    );
    const folder = comp(1, 0, { x: 0, y: 0 }, facet, [comp(9, 1, { x: 5, y: 5 })]);
    const { updates } = planPaste([folder], 0, { x: 0, y: 0 }, {
      uidMap: { components: { 1: 1001, 9: 9009 }, properties: { 200: 2002, 88: 8808 } },
    });
    const remapped = parseFacet(updates.find((u) => u.uid === 1)!.properties![FACET_PROP].value);
    const rec = remapped.get(2002);
    expect(rec?.chain).toBe(true);
    expect(rec?.childComponent).toBe(9009); // the COPY of the inner folder
    expect(rec?.facetProp).toBe(8808);
    expect(rec?.expose).toBe("output");
  });

  it("leaves an unmapped ref unchanged (incomplete uidMap is a no-op for that ref)", () => {
    const facet = serializeFacet(new Map([[200, { expose: "input", childComponent: 50, facetProp: 60 }]]));
    const { updates } = planPaste([comp(1, 0, { x: 0, y: 0 }, facet)], 0, { x: 0, y: 0 }, {
      uidMap: {
        components: {}, // 50 not mapped
        properties: { 200: 2002 }, // 60 not mapped
      },
    });
    const rec = parseFacet(updates.find((u) => u.uid === 1)!.properties![FACET_PROP].value).get(2002);
    expect(rec?.childComponent).toBe(50); // left as-is when absent from the map
    expect(rec?.facetProp).toBe(60);
  });

  it("leaves __facets untouched when no uidMap is provided", () => {
    const facet = serializeFacet(new Map([[200, { expose: "input", childComponent: 50 }]]));
    const { updates } = planPaste([comp(1, 0, { x: 0, y: 0 }, facet)], 0, { x: 0, y: 0 });
    expect(updates.find((u) => u.uid === 1)?.properties).toBeUndefined();
  });
});
