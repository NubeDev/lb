// Pure-logic unit tests for the Flows source-picker group (flow-dashboard-binding-ux-scope). The
// picker drills flow → node → port and resolves each port to the right binding — an INPUT port to a
// write `flows.inject` Action, an OUTPUT port to a `flows.node_state` read Source — with a friendly
// label, never a raw tool name. The real-gateway round-trip lives in *.gateway.test.tsx.

import { describe, expect, it } from "vitest";

import { flowsEntries, buildSourceEntries } from "./sourcePicker";
import { extractFlowValue } from "../views/useFlowNodeValue";
import { flowBindingOfAction, flowBindingOfSource } from "../views/flowBinding";
import type { Flow, NodeDescriptor } from "@/lib/flows/flows.types";

const descriptors: NodeDescriptor[] = [
  {
    type: "trigger",
    title: "Trigger",
    category: "Flow",
    kind: "trigger",
    tool: "",
    inputs: [],
    outputs: ["payload", "topic"],
    configVersion: 1,
    config: {},
  },
  {
    type: "rhai",
    title: "Rhai",
    category: "Flow",
    kind: "transform",
    tool: "rules.eval",
    inputs: ["payload"],
    outputs: ["payload", "topic", "findings"],
    configVersion: 1,
    config: {},
  },
];

function flow(): Flow {
  return {
    id: "cooler-ctl",
    name: "Cooler Control",
    version: 1,
    nodes: [
      { id: "setpoint-in", type: "rhai", needs: [], with: {}, config: { source: "payload" } },
    ],
  } as Flow;
}

describe("flows source picker group", () => {
  it("resolves an INPUT port to a port-aware flows.inject Action (no tool name shown)", () => {
    const entries = flowsEntries([flow()], descriptors);
    const input = entries.find((e) => e.id.startsWith("flows:in:"));
    expect(input?.group).toBe("flows");
    expect(input?.writes).toBe(true);
    // friendly label: "Cooler Control › setpoint-in › payload (input)" — never `flows.inject`.
    expect(input?.label).toBe("Cooler Control › setpoint-in › payload (input)");
    expect(input?.label).not.toContain("flows.inject");
    expect(input?.action).toEqual({
      tool: "flows.inject",
      argsTemplate: {
        id: "cooler-ctl",
        node: "setpoint-in",
        port: "payload",
        value: "{{value}}",
      },
    });
  });

  it("resolves an OUTPUT port to a flows.node_state read Source", () => {
    const entries = flowsEntries([flow()], descriptors);
    const out = entries.find((e) => e.id === "flows:out:cooler-ctl:setpoint-in:payload");
    expect(out?.writes).toBe(false);
    expect(out?.label).toBe("Cooler Control › setpoint-in › payload (output)");
    expect(out?.source).toEqual({
      tool: "flows.node_state",
      args: { id: "cooler-ctl", __flowNode: "setpoint-in", __flowPort: "payload" },
    });
  });

  it("skips a node whose descriptor is missing (honest empty, never a guess)", () => {
    const f = flow();
    f.nodes = [{ id: "x", type: "unknown.type", needs: [], with: {}, config: {} }];
    expect(flowsEntries([f], descriptors)).toHaveLength(0);
  });

  it("is AGNOSTIC to the node type + port names — a brand-new dev node 'just works'", () => {
    // A node type that does NOT exist today (a developer ships it tomorrow) with NON-`payload` ports.
    const devDescriptors: NodeDescriptor[] = [
      {
        type: "acme.thermostat",
        title: "Thermostat",
        category: "ACME",
        kind: "transform",
        tool: "acme.thermostat",
        inputs: ["setpoint", "mode"],
        outputs: ["temperature", "humidity"],
        configVersion: 1,
        config: {},
      },
    ];
    const f = flow();
    f.nodes = [{ id: "t1", type: "acme.thermostat", needs: [], with: {}, config: {} }];
    const entries = flowsEntries([f], devDescriptors);

    // Every declared input → a port-aware inject Action keyed on the REAL port name (no `payload`).
    const setpoint = entries.find((e) => e.id === "flows:in:cooler-ctl:t1:setpoint");
    expect(setpoint?.action).toEqual({
      tool: "flows.inject",
      argsTemplate: { id: "cooler-ctl", node: "t1", port: "setpoint", value: "{{value}}" },
    });
    expect(entries.some((e) => e.id === "flows:in:cooler-ctl:t1:mode")).toBe(true);
    // Every declared output → a node_state read Source keyed on the REAL port name.
    const temp = entries.find((e) => e.id === "flows:out:cooler-ctl:t1:temperature");
    expect(temp?.source?.args).toEqual({ id: "cooler-ctl", __flowNode: "t1", __flowPort: "temperature" });
    expect(entries.some((e) => e.id === "flows:out:cooler-ctl:t1:humidity")).toBe(true);
    // No hardcoded type/port list: exactly 2 inputs + 2 outputs surfaced.
    expect(entries).toHaveLength(4);

    // And the read-back extracts the SELECTED named output port, not `payload`.
    const entry = { node: "t1", value: { temperature: 21.5, humidity: 40 }, rev: 1 };
    expect(extractFlowValue(entry, "output", "temperature")).toBe(21.5);
    expect(extractFlowValue(entry, "output", "humidity")).toBe(40);
    // a named input port read-back (per-port retained), agnostic to the name.
    const inEntry = { node: "t1", value: null, rev: 1, inputs: { setpoint: 18 } };
    expect(extractFlowValue(inEntry, "input", "setpoint")).toBe(18);
  });

  it("buildSourceEntries folds the Flows group in alongside series/ext", () => {
    const entries = buildSourceEntries(["s"], [], [flow()], descriptors);
    expect(entries.some((e) => e.group === "flows")).toBe(true);
    expect(entries.some((e) => e.group === "series")).toBe(true);
  });
});

describe("flow binding recovery + value extraction", () => {
  it("recovers {flowId,node,port} from an inject Action and a node_state Source", () => {
    const [input, , output] = (() => {
      const entries = flowsEntries([flow()], descriptors);
      const i = entries.find((e) => e.id.startsWith("flows:in:"))!;
      const o = entries.find((e) => e.id.startsWith("flows:out:"))!;
      return [i, null, o] as const;
    })();
    expect(flowBindingOfAction(input.action)).toEqual({
      flowId: "cooler-ctl",
      node: "setpoint-in",
      port: "payload",
    });
    expect(flowBindingOfSource(output.source)).toEqual({
      flowId: "cooler-ctl",
      node: "setpoint-in",
      port: "payload",
      path: undefined, // no JSON path picked yet (the picker's entry has no __flowPath)
    });
  });

  it("a control reads its OWN input (per-port wins over node-level), a view reads the output payload", () => {
    const entry = {
      node: "setpoint-in",
      value: { payload: 42, topic: "t" },
      rev: 3,
      input: 4,
      inputs: { payload: 6 },
    };
    // input read-back: per-port `inputs.payload` (6) beats node-level `input` (4).
    expect(extractFlowValue(entry, "input", "payload")).toBe(6);
    // node-level fallback when no per-port value for the slot.
    expect(extractFlowValue({ ...entry, inputs: {} }, "input", "payload")).toBe(4);
    // output read: the envelope's `payload` field, not the whole envelope.
    expect(extractFlowValue(entry, "output", "payload")).toBe(42);
    // envelope view: the whole recorded value.
    expect(extractFlowValue(entry, "output-envelope", "payload")).toEqual({ payload: 42, topic: "t" });
  });
});
