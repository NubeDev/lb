import { describe, expect, it } from "vitest";
import { partitionEdges, exposedPortIndex, classifyCrossEdge } from "./routing";
import { serializeFacet, FACET_PROP } from "./facet";
import type { Component, Edge, Property } from "./engine-types";

function prop(uid: number, componentUid: number): Property {
  return { uid, componentUid, category: 0, value: 0, statusFlags: 0 };
}
function comp(uid: number, props: Record<string, number>, facet?: string): Component {
  const properties: Record<string, Property> = {};
  for (const [n, p] of Object.entries(props)) properties[n] = prop(p, uid);
  if (facet != null) {
    properties[FACET_PROP] = { ...prop(900 + uid, uid), value: facet, systemRole: 2 };
  }
  return { uid, name: `c${uid}`, type: "math::add", path: `root/c${uid}`, parent: 0, properties };
}
function edge(p: Partial<Edge> & { uid: number; sourceUid: number; targetUid: number }): Edge {
  return {
    sourceProperty: "out",
    targetProperty: "in",
    ...p,
  } as Edge;
}

describe("partitionEdges", () => {
  const childUids = new Set([1, 2]);
  it("node↔node is in, node↔off is cross, off↔off is dropped", () => {
    const { inEdges, crossEdges } = partitionEdges(
      [
        edge({ uid: 100, sourceUid: 1, sourcePropertyUid: 11, targetUid: 2, targetPropertyUid: 21 }), // both nodes → in
        edge({ uid: 101, sourceUid: 1, sourcePropertyUid: 11, targetUid: 9, targetPropertyUid: 91 }), // one node → cross
        edge({ uid: 102, sourceUid: 8, sourcePropertyUid: 81, targetUid: 2, targetPropertyUid: 21 }), // one node → cross
        edge({ uid: 103, sourceUid: 7, sourcePropertyUid: 71, targetUid: 9, targetPropertyUid: 91 }), // none → dropped
      ],
      childUids,
    );
    expect(inEdges.map((e) => e.uid)).toEqual([100]);
    expect(crossEdges.map((e) => e.uid)).toEqual([101, 102]);
  });

  it("an edge touching a folder PORT (both ends off-canvas components) is a crossEdge", () => {
    // folder 1 exposes deep child 50's prop 500. Edge 9.out -> 50.in(500): neither
    // 9 nor 50 is a direct child, but 500 is folder 1's port → routable → cross.
    const index = new Map([[500, { parentUid: 1 }]]);
    const { inEdges, crossEdges } = partitionEdges(
      [edge({ uid: 200, sourceUid: 9, sourcePropertyUid: 91, targetUid: 50, targetPropertyUid: 500 })],
      childUids,
      index,
    );
    expect(inEdges).toHaveLength(0);
    expect(crossEdges.map((e) => e.uid)).toEqual([200]);
  });

  it("keeps a port edge that leaves the folder (boundary)", () => {
    // folder 1 exposes deep child 50's prop 500; edge 50.out(500) -> 2.in. The
    // engine classifies this `boundary`; partition routes it as a crossEdge.
    const index = new Map([[500, { parentUid: 1 }]]);
    const { crossEdges } = partitionEdges(
      [edge({ uid: 301, sourceUid: 50, sourcePropertyUid: 500, targetUid: 2, targetPropertyUid: 21 })],
      childUids,
      index,
    );
    expect(crossEdges.map((e) => e.uid)).toEqual([301]);
  });
});

// Internal-to-one-folder edges (the folder-loopback case) are dropped UPSTREAM by
// the caller filtering the engine's `class === "internal"` (GET /edges?subtree=);
// partition itself no longer reconstructs container membership.

describe("exposedPortIndex", () => {
  it("indexes ports + the prop-subscription set", () => {
    // folder c1 exposes child prop 500 (owned by component 50, child __facets 60)
    const facet = serializeFacet(
      new Map([[500, { expose: "input", childComponent: 50, facetProp: 60 }]]),
    );
    const { index, subProps } = exposedPortIndex([comp(1, {}, facet), comp(2, { in: 21 })]);
    expect(index.get(500)).toEqual({ parentUid: 1 });
    // subscribe both the port value (500) and the child's live __facets (60)
    expect([...subProps].sort((a, b) => a - b)).toEqual([60, 500]);
  });

  it("indexes + subscribes a CHAINED port (owner resolved by the engine on wire)", () => {
    // folder c1 re-projects an inner folder (99)'s already-exposed port 500
    const facet = serializeFacet(
      new Map([[500, { expose: "input", childComponent: 99, facetProp: 88, chain: true }]]),
    );
    const { index, subProps } = exposedPortIndex([comp(1, {}, facet)]);
    expect(index.get(500)).toEqual({ parentUid: 1 }); // existing edges still route to the port
    expect([...subProps].sort((a, b) => a - b)).toEqual([88, 500]); // value + inner folder's facets
  });
});


describe("classifyCrossEdge", () => {
  const childUids = new Set([1]); // only c1 is visible
  // c1 exposes child prop 500 as a port
  const index = new Map([[500, { parentUid: 1 }]]);

  it("node → folder port = a normal edge to the folder's port handle (visible = source)", () => {
    const r = classifyCrossEdge(
      edge({ uid: 100, sourceUid: 1, sourcePropertyUid: 11, targetUid: 50, targetPropertyUid: 500 }),
      childUids,
      index,
    );
    expect(r).toMatchObject({
      kind: "edge",
      source: { uid: 1, handle: 11 },
      target: { uid: 1, handle: 500 }, // the port draws on folder 1, handle 500
    });
  });

  it("folder port → node = a normal edge (visible end is the target node)", () => {
    const r = classifyCrossEdge(
      edge({ uid: 101, sourceUid: 50, sourcePropertyUid: 500, targetUid: 1, targetPropertyUid: 12 }),
      childUids,
      index,
    );
    expect(r).toMatchObject({ kind: "edge", source: { uid: 1, handle: 500 }, target: { uid: 1, handle: 12 } });
  });

  it("folder port → off-canvas = a ghost anchored on the folder PORT", () => {
    // c2 also visible; it exposes deep child 60's prop 600. Edge 600 -> 9(off).
    const idx2 = new Map([[600, { parentUid: 2 }]]);
    const r = classifyCrossEdge(
      edge({
        uid: 104,
        sourceUid: 60,
        sourceProperty: "out",
        sourcePropertyUid: 600,
        targetUid: 9,
        targetProperty: "in",
        targetPropertyUid: 999,
        targetPath: "root/elsewhere",
      }),
      new Set([2]),
      idx2,
    );
    expect(r).toMatchObject({
      kind: "ghost",
      side: "input", // visible (port) is the source → ghost on the output side
      visibleUid: 2,
      visibleHandle: 600,
      visibleIsPort: true,
      externalUid: 9,
      externalPropName: "in",
      externalPath: "root/elsewhere",
    });
  });

  it("port → port = a normal edge between two folder port handles", () => {
    // folder 1 exposes 500, folder 2 exposes 600; edge 50.500 -> 60.600.
    const idx = new Map([
      [500, { parentUid: 1 }],
      [600, { parentUid: 2 }],
    ]);
    const r = classifyCrossEdge(
      edge({ uid: 105, sourceUid: 50, sourcePropertyUid: 500, targetUid: 60, targetPropertyUid: 600 }),
      new Set([1, 2]),
      idx,
    );
    expect(r).toMatchObject({ kind: "edge", source: { uid: 1, handle: 500 }, target: { uid: 2, handle: 600 } });
  });

  it("falls back to a ghost when the off-canvas prop isn't exposed here", () => {
    const r = classifyCrossEdge(
      edge({
        uid: 102,
        sourceUid: 1,
        sourceProperty: "out",
        sourcePropertyUid: 11,
        targetUid: 9,
        targetProperty: "in",
        targetPropertyUid: 999,
        targetPath: "root/elsewhere",
      }),
      childUids,
      index,
    );
    expect(r).toMatchObject({
      kind: "ghost",
      side: "input", // visible end (source) drives an output into off-canvas input
      visibleUid: 1,
      visibleHandle: 11,
      visiblePropName: "out",
      visibleIsPort: false,
      externalUid: 9,
      externalPropName: "in",
      externalPath: "root/elsewhere",
    });
  });

  it("carries the loopBack flag through", () => {
    const r = classifyCrossEdge(
      edge({ uid: 103, sourceUid: 1, sourcePropertyUid: 11, targetUid: 9, targetPropertyUid: 7, loopBack: true }),
      childUids,
      index,
    );
    expect(r.loopBack).toBe(true);
  });
});
