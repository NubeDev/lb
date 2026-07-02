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

  it("upsertComponent tolerates a component with NO properties (add-response shape)", () => {
    // Regression: the engine's add response (POST /nodes) can omit `properties` on a
    // brand-new component. The store indexed it with a bare `Object.values(c.properties)`,
    // which threw "Cannot convert undefined or null to object" — crashing the add-node
    // canvas action. A property-less component must upsert cleanly.
    const bare = {
      uid: 7,
      name: "fresh",
      type: "math::add",
      path: "root/fresh",
      parent: 0,
    } as unknown as Component; // deliberately missing `properties` — the raw add-response shape
    expect(() => S().upsertComponent(bare)).not.toThrow();
    expect(S().components.get(7)?.name).toBe("fresh");
    // A later re-upsert (which unindexes the prior bare entry) must also not throw.
    expect(() => S().upsertComponent(comp(7, "fresh", 0, [71]))).not.toThrow();
    expect(propertyToComponent.get(71)).toBe(7);
  });

  it("upsertEdge / removeEdge mutate only the edge map", () => {
    S().setNodes([comp(1, "a", 0, [11]), comp(2, "b", 0, [21])], []);
    S().upsertEdge(edge(100, 1, 2));
    expect(S().edges.get(100)?.sourceUid).toBe(1);
    S().removeEdge(100);
    expect(S().edges.has(100)).toBe(false);
  });
});
