// Unit tests for the canvas ⇄ record serialization (flow-runtime-control-scope regression). The
// headline check: a flow's edges survive the export round-trip — `flowToEdges` (load) →
// `nodesToFlowNodes` (the inverse `buildFlow`/export uses) must preserve every `needs` connection.
// This is the "export can't see node connections" report turned into a guard.

import { describe, expect, it } from "vitest";

import {
  effectiveInputPorts,
  flowToEdges,
  flowToNodes,
  isTerminalStatus,
  joinOf,
  lockedNodeIds,
  nodeStateValues,
  nodesToFlowNodes,
  snapshotValues,
} from "./flowGraph";
import type { Flow, FlowNodeState, FlowRunSnapshot, NodeDescriptor } from "@/lib/flows";

describe("nodeStateValues (persistent runtime view)", () => {
  it("maps each node's envelope to its payload badge, omitting null values", () => {
    const state: FlowNodeState = {
      flowId: "f",
      nodes: [
        // the stored value is the whole envelope; the badge shows its `payload` (D10).
        { node: "a", value: { payload: 4, topic: "kfc.temp" }, rev: 7 },
        // no `payload` key → fall back to the whole value.
        { node: "fallback", value: { other: 1 }, rev: 3 },
        { node: "b", value: null, rev: null }, // never ran → omitted (renders blank, not null)
      ],
    };
    const v = nodeStateValues(state);
    expect(v.a).toEqual({ output: 4, error: null });
    expect(v.fallback).toEqual({ output: { other: 1 }, error: null });
    expect(v.b).toBeUndefined();
  });
});

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

  it("loads a stored node position onto the canvas (geometry persists — the drag bug fix)", () => {
    const positioned: Flow = {
      ...FLOW,
      nodes: [
        { id: "start", type: "trigger", needs: [], config: {}, position: { x: 700, y: 40 } },
        ...FLOW.nodes.slice(1),
      ],
    };
    const canvas = flowToNodes(positioned);
    // The stored position wins over the grid fallback.
    expect(canvas.find((n) => n.id === "start")!.position).toEqual({ x: 700, y: 40 });
  });

  it("falls back to the grid layout for a node with no stored position (pre-geometry flow)", () => {
    // FLOW's nodes carry no `position` — each gets the deterministic grid slot, not undefined.
    const canvas = flowToNodes(FLOW);
    expect(canvas[0].position).toEqual({ x: 0, y: 0 });
    expect(canvas[1].position).toEqual({ x: 240, y: 0 });
  });

  it("serializes a dragged position back on save, rounded to whole pixels", () => {
    const nodes = flowToNodes(FLOW);
    // Simulate a drag: move `a` to a fractional coordinate (React Flow reports sub-pixel deltas).
    const dragged = nodes.map((n) =>
      n.id === "a" ? { ...n, position: { x: 512.4, y: 128.6 } } : n,
    );
    const out = nodesToFlowNodes(dragged, flowToEdges(FLOW), FLOW);
    expect(out.find((n) => n.id === "a")!.position).toEqual({ x: 512, y: 129 });
  });

  // --- flow-input-ports-scope Axis 1: a wire's target input port round-trips ---

  it("loads a stored `toPort` onto the edge's targetHandle", () => {
    // `b` wires `a` to a named input port `secondary`; `start` lands on the primary (no entry).
    const flow: Flow = {
      ...FLOW,
      nodes: [
        { id: "start", type: "trigger", needs: [], config: {} },
        { id: "a", type: "count", needs: ["start"], config: {} },
        {
          id: "b",
          type: "count",
          needs: ["a", "start"],
          config: {},
          inputs: [{ from: "a", toPort: "secondary" }],
        },
      ],
    };
    const edges = flowToEdges(flow);
    const byId = Object.fromEntries(edges.map((e) => [`${e.source}->${e.target}`, e]));
    // The named-port wire carries its `targetHandle`; the primary wire's handle is null.
    expect(byId["a->b"].targetHandle).toBe("secondary");
    expect(byId["start->b"].targetHandle).toBeNull();
  });

  it("round-trips a named `toPort` through load → export (and omits primary entries)", () => {
    const flow: Flow = {
      ...FLOW,
      nodes: [
        { id: "start", type: "trigger", needs: [], config: {} },
        { id: "a", type: "count", needs: ["start"], config: {} },
        {
          id: "b",
          type: "count",
          needs: ["a", "start"],
          config: {},
          inputs: [{ from: "a", toPort: "secondary" }],
        },
      ],
    };
    const out = nodesToFlowNodes(flowToNodes(flow), flowToEdges(flow), flow);
    const b = out.find((n) => n.id === "b")!;
    // Only the non-primary wire appears in `inputs` (the primary wire stays implicit).
    expect(b.inputs).toEqual([{ from: "a", toPort: "secondary" }]);
    // And the inverse back to canvas edges preserves the handle.
    const edgesBack = flowToEdges({
      ...flow,
      nodes: out as unknown as typeof flow.nodes,
    });
    const handle = Object.fromEntries(
      edgesBack.map((e) => [`${e.source}->${e.target}`, e.targetHandle]),
    );
    expect(handle["a->b"]).toBe("secondary");
    expect(handle["start->b"]).toBeNull();
  });

  it("a pre-ports flow (no `inputs`) round-trips with primary handles and no `inputs` field", () => {
    // The clean back-compat shape: FLOW has no `inputs`, so every edge is the primary port and the
    // exported node carries no `inputs` field (the honest primary-only shape).
    const out = nodesToFlowNodes(flowToNodes(FLOW), flowToEdges(FLOW), FLOW);
    for (const n of out) {
      expect(n.inputs).toBeUndefined();
    }
    const edges = flowToEdges(FLOW);
    expect(edges.every((e) => e.targetHandle == null)).toBe(true);
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
        // a settled node's recorded value is its envelope; the badge shows the `payload` (D10).
        { id: "a", claim: "done", terminal: true, outcome: "ok", output: { payload: 2 }, error: null },
        { id: "b", claim: "running", terminal: false, outcome: "", output: null, error: null },
      ],
    };
    const v = snapshotValues(snap);
    expect(v.a.output).toEqual(2);
    expect(v.b.output).toBeNull();
  });
});

// Regression (the "must refresh the page to edit a node" report): the editor lock must release the
// instant a run goes terminal — on Stop, and between an armed cron flow's finite firings. The lock is
// derived purely from the snapshot, so this is the guard for that derivation.
describe("lockedNodeIds (the executed-node editor lock)", () => {
  /** A snapshot with one executed node `a` and one un-run node `b`, at `status`. */
  const snapAt = (status: string): FlowRunSnapshot => ({
    runId: "r",
    flowId: "f",
    flowVersion: 1,
    status,
    steps: [
      { id: "a", claim: "done", terminal: true, outcome: "ok", output: null, error: null },
      { id: "b", claim: "pending", terminal: false, outcome: "", output: null, error: null },
    ],
  });

  it("locks executed nodes while a run is genuinely IN FLIGHT", () => {
    expect([...lockedNodeIds(snapAt("running"))]).toEqual(["a"]);
  });

  it("locks NOTHING once the run is terminal — so Stop/cancel frees editing with no refresh", () => {
    for (const status of ["success", "partialFailure", "failed", "cancelled"]) {
      expect(lockedNodeIds(snapAt(status)).size).toBe(0);
    }
  });

  it("locks nothing with no snapshot (a fresh canvas, or between cron firings before reattach)", () => {
    expect(lockedNodeIds(null).size).toBe(0);
  });

  it("isTerminalStatus classifies the four settled statuses, not a live one", () => {
    expect(isTerminalStatus("running")).toBe(false);
    expect(isTerminalStatus("cancelled")).toBe(true);
  });
});

// ───────────────── flow-input-ports-scope Slice 4: the canvas paints ports + policy ──────────────

const desc = (
  type: string,
  kind: NodeDescriptor["kind"],
  inputs: string[],
  inputPorts?: NodeDescriptor["inputPorts"],
): NodeDescriptor => ({
  type,
  title: type,
  category: "Flow",
  kind,
  tool: "",
  inputs,
  outputs: [],
  inputPorts,
  configVersion: 1,
  config: {},
});

describe("joinOf + effectiveInputPorts (the per-port policy the canvas paints)", () => {
  it("defaults `any` for EVERY kind (plain per-message wiring, flow-plain-wiring-scope)", () => {
    const rhai = desc("rhai", "transform", ["payload"]);
    expect(joinOf(rhai)).toBe("any");
    expect(joinOf(rhai, "payload")).toBe("any");
    const dbg = desc("debug", "sink", ["payload"]);
    expect(joinOf(dbg)).toBe("any");
    expect(joinOf(dbg, "payload")).toBe("any");
  });

  it("an explicit `inputPorts` entry declaring `all` opts the port into the barrier", () => {
    // The opt-in (an extension descriptor may declare it; no built-in does).
    const joiner = desc("joiner", "transform", ["payload"], [{ name: "payload", join: "all" }]);
    expect(joinOf(joiner)).toBe("all");
    expect(joinOf(joiner, "payload")).toBe("all");
    // A redundant explicit `any` is honoured too.
    const funnel = desc("funnel", "sink", ["payload"], [{ name: "payload", join: "any" }]);
    expect(joinOf(funnel, "payload")).toBe("any");
  });

  it("resolves the primary (first) port when no port name is given", () => {
    const multi = desc("multi", "transform", ["left", "right"]);
    expect(joinOf(multi)).toBe("any"); // left is primary — still the universal any default
    expect(joinOf(multi, "right")).toBe("any");
  });

  it("effectiveInputPorts pairs each declared port with its effective policy", () => {
    const dbg = desc("debug", "sink", ["payload"]);
    expect(effectiveInputPorts(dbg)).toEqual([{ name: "payload", join: "any" }]);
    const rhai = desc("rhai", "transform", ["payload"]);
    expect(effectiveInputPorts(rhai)).toEqual([{ name: "payload", join: "any" }]);
    // Only the explicit opt-in yields an `all` port.
    const joiner = desc("joiner", "transform", ["payload"], [{ name: "payload", join: "all" }]);
    expect(effectiveInputPorts(joiner)).toEqual([{ name: "payload", join: "all" }]);
  });

  it("a trigger/source with no inputs yields no canvas ports (no target handle)", () => {
    const trig = desc("trigger", "trigger", []);
    expect(effectiveInputPorts(trig)).toEqual([]);
  });
});

describe("flowToEdges wire-inspector label (Slice 4)", () => {
  it("labels a named-port wire with its target port; leaves primary wires clean", () => {
    const flow: Flow = {
      ...FLOW,
      nodes: [
        { id: "start", type: "trigger", needs: [], config: {} },
        { id: "a", type: "count", needs: ["start"], config: {} },
        {
          id: "b",
          type: "count",
          needs: ["a", "start"],
          config: {},
          inputs: [{ from: "a", toPort: "secondary" }],
        },
      ],
    };
    const edges = flowToEdges(flow);
    const byId = Object.fromEntries(edges.map((e) => [`${e.source}->${e.target}`, e]));
    // The named-port wire carries a midpoint label (the wire inspector surface)…
    expect(byId["a->b"].label).toBe("secondary");
    // …the primary wire stays label-free (clean canvas for the common case).
    expect(byId["start->b"].label).toBeUndefined();
  });

  it("a primary-only flow has no edge labels (the clean pre-ports canvas)", () => {
    const edges = flowToEdges(FLOW);
    expect(edges.every((e) => e.label === undefined)).toBe(true);
  });
});
