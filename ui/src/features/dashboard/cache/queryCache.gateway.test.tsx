// The dashboard read-cache de-dup, workspace-isolation, and deny behaviour — against a REAL spawned
// gateway (dashboard-query-cache-scope, testing plan; CLAUDE §9 / testing §0 — no fake backend). Each
// test logs into a UNIQUE workspace, seeds REAL series/flow records through the real write path, renders
// the SHIPPED panel views under the SHIPPED `DashboardCacheProvider` (the same boundary the dashboard
// route + channel response mount), and instruments the ONE ipc seam (`invoke`) to COUNT calls per verb.
//
// The whole point of the scope is fewer round-trips with NO behavioural change, so these tests assert the
// call COUNTS the scope promised:
//   - N cells sharing one viz.query spec → ONE `mcp_call{viz.query}` (not N); a title-only difference does
//     NOT add a call (the key is off the canonical spec, not the whole panel);
//   - N cells on one flow → ONE `flows_node_state` read (each cell slices its own node/port client-side);
//   - workspace isolation: priming ws-A's cache never serves ws-B (keys are ws-prefixed) — B reads B;
//   - a session without the read cap → an HONEST denied state, never a fabricated value.

import { describe, it, expect, beforeAll, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";

import type { Cell, Flow } from "@/lib/dashboard";
import { StatPanel } from "../views/stat/StatPanel";
import { WidgetView } from "../views/WidgetView";
import { WithDashboardCache } from "./testCacheWrapper";
import * as ipc from "@/lib/ipc/invoke";
import {
  useRealGateway,
  signInReal,
  signInWithCaps,
  seedSeries,
} from "@/test/gateway-session";
import { saveFlow, injectFlow, runFlow, getFlowRun } from "@/lib/flows/flows.api";

let n = 0;
const nextWs = () => `qcache-${n++}`;

beforeAll(() => useRealGateway());

/** Seed one real sample so a `series.read` / `viz.query` over `series` returns a value (not empty). */
async function seedSample(series: string, value: number): Promise<void> {
  await seedSeries({ series, seq: 1, payload: value, key: "kind", value: "temperature" });
}

/** A v3 stat cell over a `series.read` source resolved via `viz.query`. `i`/`title` vary per cell; the
 *  QUERY SPEC (the source) is identical across cells so they SHARE one cache entry. */
function statCell(i: string, series: string, title = ""): Cell {
  return {
    i, x: 0, y: 0, w: 4, h: 3, v: 3, widget_type: "stat", view: "stat",
    title,
    binding: { series: "" },
    sources: [{ refId: "A", tool: "series.read", args: { series }, datasource: { type: "series" } }],
    options: { reduceOptions: { calcs: ["last"] } },
  } as Cell;
}

describe("dashboard read cache — de-dup, isolation, deny (real gateway)", () => {
  it("N cells sharing one viz.query spec issue ONE viz.query; a title-only difference adds none", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedSample("cache.temp", 21);

    // Three cells, same source spec, DIFFERENT ids + titles — one shared cache entry under one provider.
    const cells = [statCell("a", "cache.temp", "Alpha"), statCell("b", "cache.temp", "Beta"), statCell("c", "cache.temp", "")];
    const counted = viaCounter(async () =>
      render(
        <WithDashboardCache ws={ws}>
          <div>
            {cells.map((c) => (
              <StatPanel key={c.i} cell={c} label={c.i} />
            ))}
          </div>
        </WithDashboardCache>,
      ),
    );
    await counted.rendered;
    // All three render the real seeded value (behaviour unchanged) …
    await waitFor(() => expect(screen.getAllByLabelText("stat value").length).toBe(3));
    for (const el of screen.getAllByLabelText("stat value")) expect(el.textContent).toContain("21");
    // … from exactly ONE viz.query round-trip (the de-dup the scope promised).
    expect(counted.tool("viz.query")).toBe(1);
    counted.restore();
  });

  it("two cells on one flow issue ONE flows_node_state read", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedFlowValue();

    const counted = viaCounter(async () =>
      render(
        <WithDashboardCache ws={ws}>
          <div>
            <WidgetView cell={flowCell("f1")} installed={[]} workspace={ws} />
            <WidgetView cell={flowCell("f2")} installed={[]} workspace={ws} />
          </div>
        </WithDashboardCache>,
      ),
    );
    await counted.rendered;
    await waitFor(() => expect(screen.getAllByLabelText("stat value").length).toBe(2));
    // Both cells read the SAME whole-flow node_state once, then slice client-side (scope goal 4).
    expect(counted.cmd("flows_node_state")).toBe(1);
    counted.restore();
  });

  it("is workspace isolated — priming ws-A never serves ws-B (keys are ws-prefixed)", async () => {
    // ws-A: same-named series with value 11.
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await seedSample("iso.temp", 11);
    render(
      <WithDashboardCache ws={wsA}>
        <StatPanel cell={statCell("a", "iso.temp")} label="a" />
      </WithDashboardCache>,
    );
    await waitFor(() => expect(screen.getByLabelText("stat value").textContent).toContain("11"));

    // ws-B: SAME series name, DIFFERENT value 22. A separate provider (fresh client) keyed on ws-B — the
    // host re-derives the ws from ws-B's token, and the key is ws-prefixed, so no ws-A value bleeds in.
    const wsB = nextWs();
    await signInReal("user:ben", wsB);
    await seedSample("iso.temp", 22);
    render(
      <WithDashboardCache ws={wsB}>
        <StatPanel cell={statCell("b", "iso.temp")} label="b" />
      </WithDashboardCache>,
    );
    await waitFor(() => {
      const values = screen.getAllByLabelText("stat value").map((e) => e.textContent ?? "");
      expect(values.some((v) => v.includes("22"))).toBe(true); // B sees B's value
    });
  });

  it("a session without the viz.query cap renders an HONEST denied state, never a fabricated value", async () => {
    const ws = nextWs();
    // Grant series.find but NOT viz.query — the read is denied server-side; the cache stores the honest
    // denied state, never a made-up number (CLAUDE §9).
    await signInWithCaps("user:ada", ws, ["mcp:series.find:call"]);
    render(
      <WithDashboardCache ws={ws}>
        <StatPanel cell={statCell("d", "denied.temp")} label="d" />
      </WithDashboardCache>,
    );
    // The panel shows the denied message and NO stat value.
    await waitFor(() => expect(screen.queryByText(/no access to this source/i)).toBeTruthy());
    expect(screen.queryByLabelText("stat value")).toBeNull();
  });
});

// --- helpers wiring the counter around a render (the spy must be installed BEFORE the render's reads) ---

/** Install the invoke counter, run `renderFn`, and expose the counts. The spy delegates to the REAL
 *  transport so every read still hits the gateway (observe, never fake). */
function viaCounter(renderFn: () => Promise<unknown>) {
  const real = ipc.invoke;
  const byTool = new Map<string, number>();
  const byCmd = new Map<string, number>();
  const spy = vi
    .spyOn(ipc, "invoke")
    .mockImplementation(((cmd: string, args?: Record<string, unknown>) => {
      byCmd.set(cmd, (byCmd.get(cmd) ?? 0) + 1);
      if (cmd === "mcp_call") {
        const tool = (args?.tool as string) ?? "?";
        byTool.set(tool, (byTool.get(tool) ?? 0) + 1);
      }
      return real(cmd, args);
    }) as typeof ipc.invoke);
  const rendered = renderFn();
  return {
    rendered,
    tool: (t: string) => byTool.get(t) ?? 0,
    cmd: (c: string) => byCmd.get(c) ?? 0,
    restore: () => spy.mockRestore(),
  };
}

/** A flow whose `rhai` node passes an injected `{payload}` straight through, so node_state records it. */
function nodeFlow(): Flow {
  return {
    id: "cache-flow",
    name: "Cache Flow",
    version: 1,
    failurePolicy: "halt",
    nodes: [{ id: "n", type: "rhai", needs: [], with: { payload: 0 }, config: { source: "payload" } }],
  } as unknown as Flow;
}

/** Seed a real recorded value on the flow node through the real inject → run → settle path. */
async function seedFlowValue(): Promise<void> {
  await saveFlow(nodeFlow());
  await injectFlow("cache-flow", "n", { payload: 7 }, "payload");
  const { run_id } = await runFlow("cache-flow");
  let snap = await getFlowRun(run_id);
  for (let i = 0; i < 40 && !["success", "partialFailure", "failed"].includes(snap.status); i++) {
    await new Promise((r) => setTimeout(r, 100));
    snap = await getFlowRun(run_id);
  }
}

/** A stat cell bound to the flow node's `payload` (a `flows.node_state` source, resolved client-side). */
function flowCell(i: string): Cell {
  return {
    i, x: 0, y: 0, w: 4, h: 3, v: 3, widget_type: "chart", view: "stat",
    binding: { series: "" },
    sources: [
      {
        refId: "A",
        tool: "flows.node_state",
        args: { id: "cache-flow", __flowNode: "n", __flowPort: "payload", __flowPath: ["payload"] },
        datasource: { type: "flows" },
      },
    ],
    options: { reduceOptions: { calcs: ["last"] } },
  } as Cell;
}
