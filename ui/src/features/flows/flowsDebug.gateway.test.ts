// The debug node + palette over a REAL gateway (debug-node-scope; CLAUDE §9 — no fake backend).
// Drives the real `flows.*` api against a real spawned node, seeded through the real write path.
// The SSE transport (the `/flows/{id}/debug/stream` route + the bus subject) is proven at the Rust
// layer in `rust/crates/host/tests/flows_debug_test.rs`; jsdom has no `EventSource`, so this file
// proves the *palette + run* wire: the `debug` node ships in the real registry, a flow with one
// saves green, and a run drives it to a terminal settle (the path that publishes debug motion).

import { describe, expect, it, beforeAll } from "vitest";

import { getFlowRun, listFlowNodes, runFlow, saveFlow, type Flow } from "@/lib/flows";
import { useRealGateway, signInReal } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `flows-debug-${n++}`;

beforeAll(() => useRealGateway());

/** A trigger → debug flow (auto-wire). The debug node is a terminal sink observing the wire. */
function debugFlow(id: string, label: string): Flow {
  return {
    id,
    name: id,
    version: 1,
    failurePolicy: "halt",
    nodes: [
      { id: "start", type: "trigger", needs: [], config: { mode: "manual" } },
      { id: "d", type: "debug", needs: ["start"], config: { label } },
    ],
  };
}

describe("debug node (real gateway)", () => {
  it("ships in the real palette under the Observability category", async () => {
    await signInReal("user:ada", nextWs());
    const nodes = await listFlowNodes();
    const dbg = nodes.find((d) => d.type === "debug");
    expect(dbg).toBeTruthy();
    expect(dbg?.kind).toBe("sink");
    expect(dbg?.category).toBe("Observability");
    // Sink shape: one `payload` in, no out (a terminal observer — never gates a subtree).
    expect(dbg?.inputs).toEqual(["payload"]);
    expect(dbg?.outputs).toEqual([]);
    // The config carries the format/collapse/rate knobs the panel + governor read.
    const props = (dbg?.config as Record<string, unknown> | undefined)?.properties as
      | Record<string, Record<string, unknown>>
      | undefined;
    expect(props?.format?.default).toBe("auto");
    expect(props?.collapse_bytes?.default).toBe(2048);
  });

  it("saves a flow with a debug node and runs it to a terminal settle", async () => {
    await signInReal("user:ada", nextWs());
    const saved = await saveFlow(debugFlow("dbg-flow", "dbg"));
    expect(saved.id).toBe("dbg-flow");
    expect(saved.version).toBe(1);

    const { run_id } = await runFlow("dbg-flow");
    // Bounded poll (the canvas's own shape) until the run settles.
    let snap = await getFlowRun(run_id);
    for (let i = 0; i < 20 && !["success", "partialFailure", "failed"].includes(snap.status); i++) {
      await new Promise((r) => setTimeout(r, 150));
      snap = await getFlowRun(run_id);
    }
    // The run reached terminal (the debug node settled; the motion was published on the bus even
    // though no UI subscriber was attached — fire-and-forget, proven in the Rust test).
    expect(["success", "partialFailure", "failed"]).toContain(snap.status);
    // The debug node appears in the run's per-node snapshot (it executed like any sink).
    expect(snap.steps.some((s) => s.id === "d")).toBe(true);
  });
});
