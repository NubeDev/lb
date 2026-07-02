import { describe, it, expect } from "vitest";
import { buildRfNodes, buildRfEdges, miniMapNodeColor, miniMapNodeStroke } from "./rfbuild";
import type { Component, Edge, Property } from "./engine-types";

function prop(uid: number, componentUid: number, category = 0): Property {
  return { uid, componentUid, category: category as Property["category"], value: 0, statusFlags: 0 };
}

function comp(uid: number, name: string, props: Record<string, Property>, extra: Partial<Component> = {}): Component {
  return { name, uid, type: "Add", path: `/${name}`, parent: 0, properties: props, ...extra };
}

const noop = () => {};

describe("buildRfNodes", () => {
  it("maps components to fb nodes with stable string ids", () => {
    const comps = [comp(10, "a", { in: prop(1, 10) }), comp(20, "b", { in: prop(2, 20) })];
    const nodes = buildRfNodes(comps, noop, noop);
    expect(nodes.map((n) => n.id)).toEqual(["10", "20"]);
    expect(nodes.every((n) => n.type === "fb" && n.draggable)).toBe(true);
  });

  it("carries selection and action markers into node data", () => {
    const comps = [comp(10, "a", {}, { childrenCount: 2 })];
    const nodes = buildRfNodes(comps, noop, noop, new Set(["10"]), new Set(["Add"]));
    expect(nodes[0].selected).toBe(true);
    expect(nodes[0].data.hasChildren).toBe(true);
    expect(nodes[0].data.childCount).toBe(2);
    expect(nodes[0].data.hasActions).toBe(true);
  });

  it("defaults selection/actions to false", () => {
    const nodes = buildRfNodes([comp(10, "a", {})], noop, noop);
    expect(nodes[0].selected).toBe(false);
    expect(nodes[0].data.hasActions).toBe(false);
    expect(nodes[0].data.hasChildren).toBe(false);
  });
});

describe("buildRfEdges", () => {
  const comps = [
    comp(10, "a", { out: prop(101, 10, 1) }),
    comp(20, "b", { in: prop(201, 20, 0) }),
  ];

  it("resolves handles from property uids", () => {
    const edges: Edge[] = [
      { uid: 5, sourceUid: 10, sourceProperty: "out", targetUid: 20, targetProperty: "in" },
    ];
    const out = buildRfEdges(edges, comps);
    expect(out).toHaveLength(1);
    expect(out[0]).toMatchObject({ id: "5", source: "10", target: "20", sourceHandle: "101", targetHandle: "201" });
  });

  it("skips edges whose endpoint properties are missing", () => {
    const edges: Edge[] = [
      { uid: 5, sourceUid: 10, sourceProperty: "nope", targetUid: 20, targetProperty: "in" },
    ];
    expect(buildRfEdges(edges, comps)).toHaveLength(0);
  });

  it("renders loopBack edges dashed", () => {
    const edges: Edge[] = [
      { uid: 5, sourceUid: 10, sourceProperty: "out", targetUid: 20, targetProperty: "in", loopBack: true },
    ];
    const out = buildRfEdges(edges, comps);
    expect(out[0].style?.strokeDasharray).toBe("6 4");
  });
});

describe("miniMap colors", () => {
  it("accents selected nodes", () => {
    const n = { id: "1", selected: true, position: { x: 0, y: 0 }, data: {} } as never;
    expect(miniMapNodeColor(n)).toBe("hsl(var(--cool))");
    expect(miniMapNodeStroke(n)).toBe("hsl(var(--cool))");
  });

  it("mutes ghost nodes", () => {
    const n = { id: "1", type: "ghost", position: { x: 0, y: 0 }, data: {} } as never;
    expect(miniMapNodeColor(n)).toBe("hsl(var(--muted-foreground))");
  });
});
