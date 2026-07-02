// Unit tests for the dirty comparator (flow-deploy-ux scope). A pristine re-open is CLEAN; a node add/
// delete, a rewiring, or a config change is DIRTY; serialization noise (key order, needs order,
// version/lifecycle flags) is NOT dirty.

import { describe, expect, it } from "vitest";

import type { Flow } from "@/lib/flows";
import { flowDirty } from "./flowDirty";

function flow(over: Partial<Flow> = {}): Flow {
  return {
    id: "f1",
    name: "f1",
    version: 3,
    nodes: [
      { id: "start", type: "trigger", needs: [], config: { mode: "manual" } },
      { id: "a", type: "rhai", needs: ["start"], config: { source: "1" } },
    ],
    failurePolicy: "halt",
    ...over,
  };
}

describe("flowDirty", () => {
  it("an identical graph is clean", () => {
    expect(flowDirty(flow(), flow())).toBe(false);
  });

  it("a changed node config is dirty", () => {
    const buffer = flow();
    buffer.nodes[1] = { ...buffer.nodes[1], config: { source: "2" } };
    expect(flowDirty(flow(), buffer)).toBe(true);
  });

  it("an added node is dirty", () => {
    const buffer = flow();
    buffer.nodes = [...buffer.nodes, { id: "b", type: "rhai", needs: ["a"], config: {} }];
    expect(flowDirty(flow(), buffer)).toBe(true);
  });

  it("a deleted node is dirty", () => {
    const buffer = flow();
    buffer.nodes = buffer.nodes.filter((n) => n.id !== "a");
    expect(flowDirty(flow(), buffer)).toBe(true);
  });

  it("a rewiring (changed needs) is dirty", () => {
    const buffer = flow();
    buffer.nodes[1] = { ...buffer.nodes[1], needs: [] };
    expect(flowDirty(flow(), buffer)).toBe(true);
  });

  it("config key order does NOT register as dirty", () => {
    const saved = flow({
      nodes: [
        { id: "start", type: "trigger", needs: [], config: { mode: "manual" } },
        { id: "a", type: "rhai", needs: ["start"], config: { source: "1", label: "x" } },
      ],
    });
    const buffer = flow({
      nodes: [
        { id: "start", type: "trigger", needs: [], config: { mode: "manual" } },
        { id: "a", type: "rhai", needs: ["start"], config: { label: "x", source: "1" } },
      ],
    });
    expect(flowDirty(saved, buffer)).toBe(false);
  });

  it("needs order does NOT register as dirty", () => {
    const saved = flow({
      nodes: [
        { id: "start", type: "trigger", needs: [], config: {} },
        { id: "x", type: "rhai", needs: [], config: {} },
        { id: "a", type: "rhai", needs: ["start", "x"], config: {} },
      ],
    });
    const buffer = flow({
      nodes: [
        { id: "start", type: "trigger", needs: [], config: {} },
        { id: "x", type: "rhai", needs: [], config: {} },
        { id: "a", type: "rhai", needs: ["x", "start"], config: {} },
      ],
    });
    expect(flowDirty(saved, buffer)).toBe(false);
  });

  it("version + lifecycle flags do NOT register as dirty (Deploy/Enable own those)", () => {
    expect(flowDirty(flow({ version: 1 }), flow({ version: 99, enabled: false }))).toBe(false);
  });

  it("node order does NOT register as dirty", () => {
    const saved = flow();
    const buffer = flow({ nodes: [...flow().nodes].reverse() });
    expect(flowDirty(saved, buffer)).toBe(false);
  });
});
