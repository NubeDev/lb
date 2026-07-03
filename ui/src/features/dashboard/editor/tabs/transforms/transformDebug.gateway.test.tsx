// Per-step transform debug — REAL gateway (editor-parity scope, step 7; CLAUDE §9). The one additive
// backend change (`viz.query` debug/`stopAt`) is exercised end to end: seed real rows, run a real
// pipeline stepwise through the real host, and assert the per-step snapshots (input + one per applied
// step) come back with the right shape. Also covers the mandatory capability-deny (a missing
// viz.query cap → honest denied, no fabricated steps).

import { describe, expect, it, beforeAll } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

import { useRealGateway, signInReal, signInWithCaps, seedSeries } from "@/test/gateway-session";
import type { Cell } from "@/lib/dashboard";
import { useVizSteps } from "../../../builder/useVizSteps";

let n = 0;
const nextWs = () => `dbg-${n++}`;

beforeAll(() => useRealGateway());

/** A v3 cell over a real series with a two-step pipeline (sortBy then limit). */
function cellWithPipeline(series: string): Cell {
  return {
    i: "c", x: 0, y: 0, w: 8, h: 4, v: 3, widget_type: "chart", view: "table",
    binding: { series: "" },
    sources: [{ refId: "A", tool: "series.read", args: { series }, datasource: { type: "series" } }],
    transformations: [
      { id: "sortBy", options: { sort: [{ field: "value", desc: false }] } },
      { id: "limit", options: { limitField: 2 } },
    ],
  };
}

describe("transform debug steps (real gateway)", () => {
  it("returns the input snapshot + one per applied step through the real pipeline", async () => {
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, ["mcp:viz.query:call", "mcp:series.read:call", "mcp:ingest.write:call", "mcp:tags.add:call"]);
    for (let i = 0; i < 5; i++) await seedSeries({ series: "dbg.temp", seq: i + 1, payload: 5 - i, key: "kind", value: "temperature" });

    const { result } = renderHook(() => useVizSteps(cellWithPipeline("dbg.temp"), true));
    await waitFor(() => expect(result.current.loading).toBe(false), { timeout: 4000 });
    expect(result.current.denied).toBe(false);

    // input + sortBy + limit = 3 snapshots.
    expect(result.current.steps.length).toBe(3);
    expect(result.current.steps[0].step).toBeNull(); // input
    expect(result.current.steps[1].step).toBe(0); // after sortBy
    expect(result.current.steps[2].step).toBe(1); // after limit
    // The final step's primary frame is limited to 2 rows.
    expect(result.current.steps[2].frames[0].length).toBe(2);
  });

  it("stopAt bounds the applied steps", async () => {
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, ["mcp:viz.query:call", "mcp:series.read:call", "mcp:ingest.write:call", "mcp:tags.add:call"]);
    for (let i = 0; i < 5; i++) await seedSeries({ series: "dbg2.temp", seq: i + 1, payload: 5 - i, key: "kind", value: "temperature" });

    // stopAt = 1 → input + only the first applied step (sortBy); limit never runs.
    const { result } = renderHook(() => useVizSteps(cellWithPipeline("dbg2.temp"), true, undefined, 0, 1));
    await waitFor(() => expect(result.current.loading).toBe(false), { timeout: 4000 });
    expect(result.current.steps.length).toBe(2);
    // limit didn't run → the frame still has all 5 rows.
    expect(result.current.steps[1].frames[0].length).toBe(5);
  });

  it("denies without the viz.query cap — honest denied, no fabricated steps", async () => {
    const ws = nextWs();
    // An explicit session WITHOUT the viz.query cap (dev-login would be too privileged).
    await signInWithCaps("user:ada", ws, ["mcp:dashboard.get:call"]);
    const { result } = renderHook(() => useVizSteps(cellWithPipeline("nope"), true));
    await waitFor(() => expect(result.current.loading).toBe(false), { timeout: 4000 });
    expect(result.current.denied).toBe(true);
    expect(result.current.steps).toEqual([]);
  });
});
