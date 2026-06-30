// Unit tests for the canvas ⇄ record serialization (flow-runtime-control-scope regression). The
// headline check: a flow's edges survive the export round-trip — `flowToEdges` (load) →
// `nodesToFlowNodes` (the inverse `buildFlow`/export uses) must preserve every `needs` connection.
// This is the "export can't see node connections" report turned into a guard.

import { describe, expect, it } from "vitest";

import { flowToEdges, flowToNodes, nodesToFlowNodes, snapshotValues } from "./flowGraph";
import type { Flow, FlowRunSnapshot } from "@/lib/flows";

const FLOW: Flow = {
  id: "f1",
  name: "f1",
  version: 3,
  nodes: [
    { id: "start", type: "trigger", needs: [], config: {} },
    { id: "a", type: "count", needs: ["start"], config: { k: 1 } },
    { id: "b", type: "count", needs: ["a", "start"], config: {} },
  ],
};

describe("flowGraph export round-trip", () => {
  it("preserves every `needs` connection through load → export", () => {
    const nodes = flowToNodes(FLOW);
    const edges = flowToEdges(FLOW);
    // The inverse — exactly what `buildFlow()`/export calls.
    const out = nodesToFlowNodes(nodes, edges, FLOW);

    const byId = Object.fromEntries(out.map((n) => [n.id, n]));
    expect(byId.start.needs).toEqual([]);
    expect(byId.a.needs).toEqual(["start"]);
    // A fan-in node keeps BOTH dependencies (order-independent).
    expect(new Set(byId.b.needs)).toEqual(new Set(["a", "start"]));
  });

  it("emits one edge per `needs` with stable source→target ids", () => {
    const edges = flowToEdges(FLOW);
    expect(edges).toHaveLength(3); // start→a, start→b, a→b
    expect(edges.map((e) => e.id).sort()).toEqual(["a->b", "start->a", "start->b"]);
    for (const e of edges) {
      expect(e.source).toBeTruthy();
      expect(e.target).toBeTruthy();
    }
  });

  it("preserves node type + config alongside needs", () => {
    const out = nodesToFlowNodes(flowToNodes(FLOW), flowToEdges(FLOW), FLOW);
    const a = out.find((n) => n.id === "a")!;
    expect(a.type).toBe("count");
    expect(a.config).toEqual({ k: 1 });
  });
});

describe("snapshotValues", () => {
  it("projects each settled node's output/error for the live legibility surface", () => {
    const snap: FlowRunSnapshot = {
      runId: "r",
      flowId: "f1",
      flowVersion: 3,
      status: "running",
      steps: [
        { id: "a", claim: "done", terminal: true, outcome: "ok", output: { count: 2 }, error: null },
        { id: "b", claim: "running", terminal: false, outcome: "", output: null, error: null },
      ],
    };
    const v = snapshotValues(snap);
    expect(v.a.output).toEqual({ count: 2 });
    expect(v.b.output).toBeNull();
  });
});
