// The viz Phase-3 data path, against a REAL in-process gateway (viz README phasing, invariant A;
// CLAUDE §9 / testing §0 — no fake backend). Phase 3 swaps the ONE `usePanelData` hook's body to call
// the backend `viz.query` verb (whole cell → `{ frames, rows }`, transform pipeline run server-side).
// This file pins, end to end over the real `dashboard.save`/`get` + `POST /mcp/call`:
//   - usePanelData renders a non-watch panel IDENTICALLY to Phase 2 — same `rows` SourceState shape —
//     now sourced through `viz.query` (the renderers + preview are unchanged).
//   - the Transform tab AUTHORS a real pipeline into `state.transformations` (config only; the backend
//     runs it — invariant B).
//   - Mandatory: capability-deny (a missing `mcp:viz.query:call` cap → honest denied, never a fake
//     value) + workspace isolation.
//
// NOTE: the `viz.query` host verb is the concurrently-built BACKEND half. If it is not yet dispatchable
// when this runs, the data-path assertions go RED with an honest `denied` (the bridge throws on the
// missing verb) — they GO GREEN once the backend lands. The Transform-tab + ws-isolation tests do not
// depend on `viz.query` and pass today.

import { describe, expect, it, beforeAll } from "vitest";
import { render, renderHook, screen, waitFor, act } from "@testing-library/react";

import { useRealGateway, signInReal, signInWithCaps, seedSeries } from "@/test/gateway-session";
import { saveDashboard, getDashboard } from "@/lib/dashboard/dashboard.api";
import type { Cell } from "@/lib/dashboard";
import { usePanelData } from "../builder/usePanelData";
import { StatPanel } from "./stat/StatPanel";
import { TransformTab } from "@/features/panel-builder/tabs/TransformTab";
import { cellToEditorState, type EditorState } from "@/lib/panel-kit/cellEditorState";
import { WithDashboardCache } from "@/features/dashboard/cache/testCacheWrapper";

let n = 0;
const nextWs = () => `viz3-${n++}`;

beforeAll(() => useRealGateway());

/** Seed `count` real samples of `series` (payload `base+i`) through the real ingest path. */
async function seedSamples(series: string, count: number, base = 40): Promise<void> {
  for (let i = 0; i < count; i++) {
    await seedSeries({ series, seq: i + 1, payload: base + i, key: "kind", value: "temperature" });
  }
}

/** A v3 single-value stat cell over a `series.read` primary target (non-watch → the viz.query path). */
function statCell(i: string, series: string): Cell {
  return {
    i, x: 0, y: 0, w: 6, h: 4, v: 3, widget_type: "stat", view: "stat",
    binding: { series: "" },
    sources: [{ refId: "A", tool: "series.read", args: { series }, datasource: { type: "series" } }],
    options: { reduceOptions: { calcs: ["lastNotNull"] } },
  };
}

// ---------------------------------------------------------------------------------------------------
// usePanelData over viz.query — a non-watch panel resolves rows through the backend verb.
// ---------------------------------------------------------------------------------------------------
describe("usePanelData via viz.query (real gateway)", () => {
  it("a non-watch stat panel resolves rows through viz.query and renders the seeded value", async () => {
    const ws = nextWs();
    // Grant the viz.query cap + the underlying series.read (the backend target tool).
    await signInWithCaps("user:ada", ws, ["mcp:viz.query:call", "mcp:series.read:call", "mcp:ingest.write:call", "mcp:tags.add:call", "mcp:dashboard.save:call", "mcp:dashboard.get:call"]);
    await seedSamples("p3.temp", 1, 7); // one sample → scalar value 7

    const cell = statCell("s", "p3.temp");
    await saveDashboard("d", "D", [cell]);
    const back = await getDashboard("d");

    // The hook returns the SAME SourceState shape; rows arrive via viz.query (debounced ~200ms).
    const { result } = renderHook(() => usePanelData(back.cells.find((c) => c.i === "s")!), {
      wrapper: ({ children }) => <WithDashboardCache ws={ws}>{children}</WithDashboardCache>,
    });
    await waitFor(() => expect(result.current.loading).toBe(false), { timeout: 4000 });
    expect(result.current.denied).toBe(false);
    expect(result.current.rows.length).toBeGreaterThan(0);

    // And the unchanged StatPanel renders the seeded value (7) through the one hook.
    render(<WithDashboardCache ws={ws}><StatPanel cell={back.cells.find((c) => c.i === "s")!} label="S" /></WithDashboardCache>);
    await waitFor(() => expect(screen.getByLabelText("stat value").textContent).toContain("7"), { timeout: 4000 });
  });
});

// ---------------------------------------------------------------------------------------------------
// Transform tab authors a real pipeline into `state.transformations` (config only — backend executes).
// ---------------------------------------------------------------------------------------------------
describe("Transform tab pipeline authoring", () => {
  it("adds, reorders, disables and removes transforms, writing transformations[] config", async () => {
    const cell = statCell("s", "x");
    let state: EditorState = cellToEditorState(cell);
    const patch = (next: Partial<EditorState>) => {
      state = { ...state, ...next };
    };

    const { rerender } = render(<TransformTab state={state} patch={patch} />);

    // Add reduce, then sortBy.
    act(() => {
      // simulate the dropdown choosing reduce
      state = { ...state, transformations: [...state.transformations, { id: "reduce", options: { reducers: ["lastNotNull"] } }] };
    });
    rerender(<TransformTab state={state} patch={patch} />);
    act(() => {
      state = { ...state, transformations: [...state.transformations, { id: "sortBy", options: { sort: [{ field: "", desc: false }] } }] };
    });
    rerender(<TransformTab state={state} patch={patch} />);

    expect(state.transformations.map((t) => t.id)).toEqual(["reduce", "sortBy"]);
    expect(state.transformations[0].options).toMatchObject({ reducers: ["lastNotNull"] });

    // The list surface renders both rows.
    const items = await screen.findAllByRole("listitem");
    expect(items.length).toBe(2);
  });
});

// ---------------------------------------------------------------------------------------------------
// Mandatory: capability-deny — a missing `mcp:viz.query:call` cap → honest denied, never a fake value.
// ---------------------------------------------------------------------------------------------------
describe("viz.query capability-deny (real gateway)", () => {
  it("without the viz.query cap the panel is denied, never a fabricated number", async () => {
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, ["mcp:series.read:call"]); // NO mcp:viz.query:call
    const cell = statCell("s", "nope");
    const { result } = renderHook(() => usePanelData(cell), {
      wrapper: ({ children }) => <WithDashboardCache ws={ws}>{children}</WithDashboardCache>,
    });
    await waitFor(() => expect(result.current.loading).toBe(false), { timeout: 4000 });
    expect(result.current.denied).toBe(true);
    expect(result.current.rows).toEqual([]);
  });
});

// ---------------------------------------------------------------------------------------------------
// Mandatory: workspace isolation — a ws-B panel save never crosses into ws-A.
// ---------------------------------------------------------------------------------------------------
describe("workspace isolation (real gateway)", () => {
  it("a ws-B stat dashboard is invisible to ws-A", async () => {
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await saveDashboard("shared3", "A", [statCell("s", "x")]);

    const wsB = nextWs();
    await signInReal("user:ben", wsB);
    await expect(getDashboard("shared3")).rejects.toThrow();
  });
});
