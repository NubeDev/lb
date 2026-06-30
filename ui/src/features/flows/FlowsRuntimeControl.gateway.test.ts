// The flow runtime-control surface over a REAL gateway (flow-runtime-control-scope; CLAUDE §9 — no
// fake backend). Drives the real `flows.node.get`/`flows.node.update` api + the async run against a
// real spawned node, seeded through the real write path. Covers the scope's mandatory cases:
// node-config round-trip, schema-validation reject, capability-deny per node verb, workspace-
// isolation, and the export `edges[]` round-trip (the "can't see node connections" regression — the
// canvas serialization is unit-proven in `flowGraph.test`, this proves the saved-flow shape).

import { describe, expect, it, beforeAll } from "vitest";

import {
  getFlow,
  getFlowNode,
  getFlowRun,
  runFlow,
  saveFlow,
  updateFlowNode,
  type Flow,
} from "@/lib/flows";
import { useRealGateway, signInReal, signInWithCaps } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `flows-rtc-${n++}`;

beforeAll(() => useRealGateway());

/** A two-node flow: a manual trigger → a count. */
function flow(id: string): Flow {
  return {
    id,
    name: id,
    version: 1,
    failurePolicy: "halt",
    nodes: [
      { id: "start", type: "trigger", needs: [], config: { mode: "manual" } },
      { id: "c", type: "count", needs: ["start"], config: {} },
    ],
  };
}

/** Poll a run to terminal (the run is a background job now). */
async function awaitTerminal(runId: string): Promise<string> {
  for (let i = 0; i < 200; i++) {
    const snap = await getFlowRun(runId);
    if (["success", "partialFailure", "failed", "cancelled"].includes(snap.status)) {
      return snap.status;
    }
    await new Promise((r) => setTimeout(r, 25));
  }
  throw new Error(`run ${runId} did not settle`);
}

describe("flow runtime control (real gateway)", () => {
  it("flows.node.update replaces one node's config + bumps version; node.get round-trips it", async () => {
    await signInReal("user:ada", nextWs());
    await saveFlow(flow("nc"));

    const res = await updateFlowNode("nc", "start", { mode: "manual", inject_mode: "fire" });
    expect(res.version).toBe(2); // a config edit is a new version (Decision 1)

    const got = await getFlowNode("nc", "start");
    expect(got.type).toBe("trigger");
    expect(got.config.inject_mode).toBe("fire");

    // The whole-flow read agrees (the node-level write IS the saved-flow write — no divergence).
    const back = await getFlow("nc");
    expect(back.version).toBe(2);
    expect(back.nodes.find((x) => x.id === "start")?.config).toMatchObject({ inject_mode: "fire" });
  });

  it("flows.node.update rejects a schema-invalid config (400) and leaves the record unchanged", async () => {
    await signInReal("user:ada", nextWs());
    await saveFlow(flow("bad"));
    // The `count` descriptor forbids additional properties.
    await expect(updateFlowNode("bad", "c", { not_a_field: 1 })).rejects.toThrow();
    // Untouched: still version 1, config empty.
    const back = await getFlow("bad");
    expect(back.version).toBe(1);
    expect(back.nodes.find((x) => x.id === "c")?.config ?? {}).toEqual({});
  });

  it("capability-deny: node.get / node.update refused without their caps", async () => {
    const ws = nextWs();
    // First seed a flow as a full caller, then re-sign as a gated one.
    await signInReal("user:ada", ws);
    await saveFlow(flow("gated"));

    await signInWithCaps("user:viewer", ws, [
      "mcp:flows.get:call",
      "mcp:flows.nodes:call",
      "store:flow:read",
    ]);
    await expect(getFlowNode("gated", "start")).rejects.toThrow();
    await expect(updateFlowNode("gated", "start", { mode: "manual" })).rejects.toThrow();
  });

  it("workspace isolation: a ws-B caller cannot read/edit a ws-A flow's node", async () => {
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await saveFlow(flow("walled"));

    // A different workspace — full caps in its own ws, but cannot reach ws-A's flow.
    await signInReal("user:bob", nextWs());
    await expect(getFlowNode("walled", "start")).rejects.toThrow();
    await expect(updateFlowNode("walled", "start", { mode: "manual" })).rejects.toThrow();
  });

  it("the run is a background job: runFlow returns then the run settles on its own", async () => {
    await signInReal("user:ada", nextWs());
    await saveFlow(flow("bg"));
    const { run_id } = await runFlow("bg");
    expect(run_id).toBeTruthy();
    const status = await awaitTerminal(run_id);
    expect(status).toBe("success");
  });

  it("export shape: a saved flow carries `needs` connections (the export source of truth)", async () => {
    await signInReal("user:ada", nextWs());
    await saveFlow(flow("exp"));
    // `getFlow` is exactly what the canvas exports (buildFlow serializes the same `nodes[].needs`).
    const exported = await getFlow("exp");
    const c = exported.nodes.find((x) => x.id === "c")!;
    expect(c.needs).toEqual(["start"]); // the connection survives the round-trip
  });
});
