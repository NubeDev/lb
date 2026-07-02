import { beforeEach, describe, expect, it } from "vitest";
import { useStructural, propertyToComponent } from "./store";
import type { Component, Edge, Property } from "./engine-types";

// Minimal builders so a test reads as "a component with these props" rather than
// a wall of boilerplate.
function prop(uid: number, componentUid: number): Property {
  return { uid, componentUid, category: 0, value: 0, statusFlags: 0 };
}
function comp(uid: number, name: string, parent: number, propUids: number[]): Component {
  const properties: Record<string, Property> = {};
  propUids.forEach((p, i) => (properties[`p${i}`] = prop(p, uid)));
  return { uid, name, type: "math::add", path: `root/${name}`, parent, properties };
}
function edge(uid: number, sourceUid: number, targetUid: number): Edge {
  return { uid, sourceUid, sourceProperty: "out", targetUid, targetProperty: "in" };
}

const S = () => useStructural.getState();

describe("useStructural", () => {
  beforeEach(() => {
    // setNodes replaces the maps and rebuilds the prop→component index, so it
    // doubles as a clean reset between tests.
    S().setNodes([], []);
  });

  it("indexes components by uid + path and builds the prop→component map", () => {
    S().setNodes([comp(1, "a", 0, [11, 12]), comp(2, "b", 0, [21])], []);
    expect(S().components.get(1)?.name).toBe("a");
    expect(S().componentsByPath.get("root/b")?.uid).toBe(2);
    expect(propertyToComponent.get(11)).toBe(1);
    expect(propertyToComponent.get(21)).toBe(2);
  });

  it("removeComponent cascades: drops referencing edges and unindexes its props", () => {
    S().setNodes(
      [comp(1, "a", 0, [11]), comp(2, "b", 0, [21])],
      [edge(100, 1, 2), edge(101, 2, 2)],
    );
    S().removeComponent(1);

    expect(S().components.has(1)).toBe(false);
    expect(S().componentsByPath.has("root/a")).toBe(false);
    expect(propertyToComponent.has(11)).toBe(false);
    // edge 100 touched component 1 → gone; edge 101 (entirely on 2) → kept.
    expect(S().edges.has(100)).toBe(false);
    expect(S().edges.has(101)).toBe(true);
  });

  it("upsertComponent re-indexes properties (old prop uids dropped)", () => {
    S().setNodes([comp(1, "a", 0, [11])], []);
    S().upsertComponent(comp(1, "a", 0, [12])); // prop 11 replaced by 12
    expect(propertyToComponent.has(11)).toBe(false);
    expect(propertyToComponent.get(12)).toBe(1);
  });

  it("upsertEdge / removeEdge mutate only the edge map", () => {
    S().setNodes([comp(1, "a", 0, [11]), comp(2, "b", 0, [21])], []);
    S().upsertEdge(edge(100, 1, 2));
    expect(S().edges.get(100)?.sourceUid).toBe(1);
    S().removeEdge(100);
    expect(S().edges.has(100)).toBe(false);
  });
});
