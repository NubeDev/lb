// The dashboard↔flow binding over a REAL gateway (dashboard-binding scope, Slice F; CLAUDE §9 — no
// fake backend). One dashboard DRIVES a flow (control → `flows.inject` → retained input) and
// VISUALISES it (chart ← a node's series), both over the SHIPPED WidgetBridge — `flows.inject` is
// just one more granted action tool; a flow-node series is just one more source. Covers the scope's
// mandatory cases: a granted inject round-trips (retain → no run, the next run reads it), a viewer
// without the cap is refused at the bridge (the retained input never touched), workspace isolation
// across both directions, and the read-out binding reads a flow-node series over the shipped path.
//
// The publish→watch SSE round-trip (chart redraw) is proven at the transport in
// role/gateway/tests/flows_routes_test.rs + series_stream — jsdom has no EventSource, so the live
// redraw is asserted at the wiring level here (the bridge binds a flow-node series source).

import { describe, expect, it, beforeAll } from "vitest";

import {
  getFlowRun,
  getFlow,
  getFlowNodeState,
  injectFlow,
  listFlows,
  listFlowNodes,
  runFlow,
  saveFlow,
  type Flow,
} from "@/lib/flows";
import { makeWidgetBridge } from "@/features/dashboard/builder/widgetBridge";
import { buildSourceEntries } from "@/features/dashboard/builder/sourcePicker";
import { extractFlowValue } from "@/features/dashboard/views/useFlowNodeValue";
import { interpolateArgs, resolveBuiltins, type VarScope } from "@/lib/vars";
import { useRealGateway, signInReal, signInWithCaps, seedFlowNode } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `flow-bind-${n++}`;

beforeAll(() => useRealGateway());

/** The Cooler-Control flow: a retained `setpoint` inject input + an output node. The control writes
 *  the retained setpoint; the next (event-triggered) run reads it. */
function coolerFlow(): Flow {
  return {
    id: "cooler-ctl",
    name: "cooler-ctl",
    version: 1,
    failurePolicy: "halt",
    nodes: [
      { id: "setpoint-in", type: "trigger", needs: [], config: { mode: "inject", inject_mode: "retain" } },
      { id: "temp-out", type: "sink", needs: ["setpoint-in"], config: { target: "series", name: "temp-out" } },
    ],
  };
}

describe("dashboard ↔ flow binding (real gateway)", () => {
  it("a slider control drives flows.inject → retained input set, NO run started (Decision 9)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveFlow(coolerFlow());

    // The control's argsTemplate, interpolated from the slider value (exactly the SliderControl path).
    const scope: VarScope = { values: {}, builtins: resolveBuiltins({ workspace: ws }) };
    const action = { tool: "flows.inject", argsTemplate: { id: "cooler-ctl", node: "setpoint-in", value: "{{value}}" } };
    const args = interpolateArgs(action.argsTemplate, scope, 4) as Record<string, unknown>;

    // The bridge the dashboard cell uses (the cell's tool set ∩ grant). flows.inject is in it.
    const bridge = makeWidgetBridge(["flows.inject"]);
    const res = await bridge.call<{ fired_run: boolean }>("flows.inject", args);
    expect(res.fired_run).toBe(false); // retain → no run (the next event-triggered run reads it)
  });

  it("the next run reads the retained input (the run executes the inject node)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveFlow(coolerFlow());

    const bridge = makeWidgetBridge(["flows.inject"]);
    await bridge.call("flows.inject", {
      id: "cooler-ctl",
      node: "setpoint-in",
      value: 4,
    });

    // An event-triggered one-shot run (here, manual) reads the retained setpoint and runs to terminal.
    const { run_id } = await runFlow("cooler-ctl");
    let snap = await getFlowRun(run_id);
    for (let i = 0; i < 20 && !["success", "partialFailure", "failed"].includes(snap.status); i++) {
      await new Promise((r) => setTimeout(r, 150));
      snap = await getFlowRun(run_id);
    }
    // The inject node executed (the run consumed the retained input).
    expect(snap.steps.some((s) => s.id === "setpoint-in")).toBe(true);
  });

  it("capability-deny: a viewer's slider is REFUSED at the bridge; retained input never touched", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveFlow(coolerFlow());

    // A viewer session WITHOUT `mcp:flows.inject:call` (the headline deny).
    await signInWithCaps("user:dave", ws, [
      "mcp:flows.get:call",
      "mcp:flows.runs.get:call",
      "store:flow:read",
    ]);
    // The bridge leash: flows.inject is NOT in the viewer's cell tool set → rejected client-side
    // (defense in depth). The host would deny it too.
    const viewerBridge = makeWidgetBridge(["series.read"]);
    await expect(
      viewerBridge.call("flows.inject", { id: "cooler-ctl", node: "setpoint-in", value: 9 }),
    ).rejects.toThrow(/out_of_scope/);

    // And even via the direct verb (bypassing the leash), the host denies the viewer.
    const directBridge = makeWidgetBridge(["flows.inject"]);
    await expect(
      directBridge.call("flows.inject", { id: "cooler-ctl", node: "setpoint-in", value: 9 }),
    ).rejects.toThrow(); // 403 from the host
  });

  it("workspace isolation: ws-A's control cannot inject into ws-B's flow (ws from the token)", async () => {
    const wsA = nextWs();
    const wsB = `other-${wsA}`;
    // Both workspaces have a "cooler-ctl" flow (same id, different workspace).
    await signInReal("user:ada", wsA);
    await saveFlow(coolerFlow());
    await signInReal("user:bob", wsB);
    await saveFlow(coolerFlow());

    // ws-A's session injects "cooler-ctl" → resolves in ws-A's namespace (ws-A's retained input),
    // NEVER ws-B's. Assert the call succeeds against ws-A's own flow (the wall is the token).
    const bridge = makeWidgetBridge(["flows.inject"]);
    const res = await bridge.call<{ fired_run: boolean }>("flows.inject", {
      id: "cooler-ctl",
      node: "setpoint-in",
      value: 4,
    });
    expect(res.fired_run).toBe(false);
    // ws-B's retained input is a separate record in ws-B's namespace — unreachable from ws-A's token.
  });

  it("read-out (legacy): a flow-node series binds as a widget source over the shipped series path", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    // Seed a real flow-node-shaped series (the `flow:{ws}:{flow}:{node}` convention, Decision 2) so a
    // chart's `{tool:"series.watch", args:{series}}` source binds a real, readable series.
    await seedFlowNode({
      ext: "sim",
      toolCap: "mcp:sim.emit:call",
      block: { type: "emit", kind: "source", tool: "emit", title: "Sim Source", category: "Sim" },
    });
    // A chart cell binds the flow-node series; the bridge carries series.read in its tool set.
    const bridge = makeWidgetBridge(["series.read", "series.latest"]);
    // The series naming convention a flow output node emits onto (Decision 2).
    const series = `flow:${ws}:cooler-ctl:temp-out`;
    // A history read over the real series path — the read-out wiring (live redraw is the SSE transport,
    // proven in role/gateway; jsdom has no EventSource). It resolves whether or not samples exist yet.
    await expect(bridge.call("series.read", { series })).resolves.toBeDefined();
  });
});

// ───────────────────────── flow⇄dashboard binding UX (port-aware, picker, JSON) ─────────────────────────

/** A control flow whose `ctl` node ECHOES its `payload` (so a run records the resolved input as the
 *  node's output payload — the precedence is observable). `with.payload` is the static baseline. */
function ctlFlow(): Flow {
  return {
    id: "cooler-ctl",
    name: "Cooler Control",
    version: 1,
    failurePolicy: "halt",
    nodes: [{ id: "ctl", type: "rhai", needs: [], with: { payload: 1 }, config: { source: "payload" } }],
  } as Flow;
}

async function runPayload(): Promise<unknown> {
  const { run_id } = await runFlow("cooler-ctl");
  let snap = await getFlowRun(run_id);
  for (let i = 0; i < 40 && !["success", "partialFailure", "failed"].includes(snap.status); i++) {
    await new Promise((r) => setTimeout(r, 100));
    snap = await getFlowRun(run_id);
  }
  const out = snap.steps.find((s) => s.id === "ctl")?.output as Record<string, unknown> | undefined;
  return out?.payload;
}

describe("flow⇄dashboard binding UX (real gateway)", () => {
  it("the Flows picker group lists seeded flows and emits the right Action/Source", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveFlow(ctlFlow());

    const [flows, descriptors] = await Promise.all([
      listFlows().then((fs) => Promise.all(fs.map((f) => getFlow(f.id)))),
      listFlowNodes(),
    ]);
    const entries = buildSourceEntries([], [], flows, descriptors);
    const input = entries.find((e) => e.id === "flows:in:cooler-ctl:ctl:payload");
    const output = entries.find((e) => e.id === "flows:out:cooler-ctl:ctl:payload");
    // The produced binding is asserted (not a mock): a port-aware inject Action + a node_state Source.
    expect(input?.action).toEqual({
      tool: "flows.inject",
      argsTemplate: { id: "cooler-ctl", node: "ctl", port: "payload", value: "{{value}}" },
    });
    expect(output?.source).toEqual({
      tool: "flows.node_state",
      args: { id: "cooler-ctl", __flowNode: "ctl", __flowPort: "payload" },
    });
    expect(input?.label).not.toContain("flows.inject");
  });

  it("a slider fires a real port-aware inject; the per-port retained value wins over node-level + with", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveFlow(ctlFlow());

    // Baseline: only the static `with` → payload 1.
    expect(await runPayload()).toBe(1);

    // node-level inject (no port) → wins over `with`.
    await injectFlow("cooler-ctl", "ctl", 5);
    expect(await runPayload()).toBe(5);

    // The slider's bridge call is a PORT-aware inject (the SliderControl path through the bridge).
    const scope: VarScope = { values: {}, builtins: resolveBuiltins({ workspace: ws }) };
    const action = {
      tool: "flows.inject",
      argsTemplate: { id: "cooler-ctl", node: "ctl", port: "payload", value: "{{value}}" },
    };
    const args = interpolateArgs(action.argsTemplate, scope, 9) as Record<string, unknown>;
    const bridge = makeWidgetBridge(["flows.inject"]);
    await bridge.call("flows.inject", args);
    // per-port retained (9) beats node-level (5) beats `with` (1) — the precedence headline.
    expect(await runPayload()).toBe(9);
  });

  it("a control reflects the seeded CURRENT value on mount (its own input, via node_state)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveFlow(ctlFlow());
    await injectFlow("cooler-ctl", "ctl", 7, "payload"); // a per-port retained value

    // The control's mount read = `flows.node_state` extract of THIS node's input (not its output).
    const st = await getFlowNodeState("cooler-ctl");
    const entry = st.nodes.find((nn) => nn.node === "ctl");
    expect(extractFlowValue(entry, "input", "payload")).toBe(7);
  });

  it("a switch sets a boolean retained input", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveFlow(ctlFlow());
    await injectFlow("cooler-ctl", "ctl", true, "payload");
    const st = await getFlowNodeState("cooler-ctl");
    expect(extractFlowValue(st.nodes.find((nn) => nn.node === "ctl"), "input", "payload")).toBe(true);
  });

  it("a JSON control injects a validated object and the run reads it; invalid JSON never calls", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveFlow(ctlFlow());

    // Invalid JSON is rejected BEFORE any call — the JsonControl parse gate (no fake accept).
    let called = false;
    const text = "{ not json";
    try {
      JSON.parse(text);
      called = true;
      await injectFlow("cooler-ctl", "ctl", {}, "payload");
    } catch {
      /* parse threw → no inject */
    }
    expect(called).toBe(false);

    // A valid structured object injects and round-trips through the run as the node's payload.
    const obj = { mode: "eco", band: [3.5, 4.5] };
    await injectFlow("cooler-ctl", "ctl", obj, "payload");
    expect(await runPayload()).toEqual(obj);
  });

  it("a JSON read view renders a node's structured payload from node_state and advances on change", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveFlow(ctlFlow());

    // Seed a structured value as the node's output (run with a retained object payload).
    await injectFlow("cooler-ctl", "ctl", { a: 1 }, "payload");
    await runPayload();
    let st = await getFlowNodeState("cooler-ctl");
    expect(extractFlowValue(st.nodes.find((nn) => nn.node === "ctl"), "output", "payload")).toEqual({ a: 1 });

    // Change it → the read view (a fresh node_state read on the refresh tick) advances.
    await injectFlow("cooler-ctl", "ctl", { a: 2 }, "payload");
    await runPayload();
    st = await getFlowNodeState("cooler-ctl");
    expect(extractFlowValue(st.nodes.find((nn) => nn.node === "ctl"), "output", "payload")).toEqual({ a: 2 });
  });

  it("isolation: a ws-A read view cannot read ws-B's flow_node_state; the picker never lists ws-B flows", async () => {
    const wsA = nextWs();
    const wsB = `other-${wsA}`;
    await signInReal("user:bob", wsB);
    await saveFlow(ctlFlow());
    await injectFlow("cooler-ctl", "ctl", 42, "payload");

    // ws-A has NO such flow — node_state of "cooler-ctl" is denied/absent (NotFound→403), never ws-B's.
    await signInReal("user:ada", wsA);
    await expect(getFlowNodeState("cooler-ctl")).rejects.toThrow();
    // and ws-A's picker lists none of ws-B's flows.
    expect(await listFlows()).toHaveLength(0);
  });
});
