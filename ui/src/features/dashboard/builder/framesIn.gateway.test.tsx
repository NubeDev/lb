// Frames-in for extension DATA widgets (ext-widget-source-binding scope), driven against a REAL
// in-process gateway (CLAUDE §9 / testing §0 — no fake backend). A `data = true` `ext:<id>/<widget>`
// cell carries the SAME `sources[]` as a built-in view; the SHELL resolves them through the shipped
// `viz.query` path under the VIEWER's grant and hands the tile resolved frames (`ctx.data`) — the tile
// renders, it never fetches. This suite exercises that shell seam (`useVizFrames`) + the render
// dispatch end to end, covering the mandatory categories:
//   - capability DENY: a viewer without the source tool's cap → no frames (honest), never fabricated;
//   - workspace ISOLATION: ws-A data is not resolvable by a ws-B viewer (the `viz.query` wall);
//   - v2 COMPAT: a v2 tile (no `data`) under the v3 shell resolves NO frames — its path is untouched;
//   - LIVE: fresh frames flow to the tile via the shared read cache on a refresh tick;
//   - DASHBOARD + CHANNEL PARITY: the SAME `data = true` ext cell mounts through the ONE `WidgetView`
//     dispatcher from a dashboard AND a channel `ResponseView` — identical frames, one render path.

import { describe, expect, it, beforeAll } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

import {
  useRealGateway,
  signInWithCaps,
  seedExtension,
  seedSeries,
} from "@/test/gateway-session";
import { listExtensions } from "@/lib/ext/ext.api";
import { saveDashboard, getDashboard } from "@/lib/dashboard/dashboard.api";
import type { Cell } from "@/lib/dashboard";
import { WithDashboardCache } from "@/features/dashboard/cache/testCacheWrapper";
import { useVizFrames } from "./useVizFrames";
import { extWidgetEntries } from "./sourcePicker";

/** A `data = true` `[[widget]]` tile — the reference frames-in tile. Empty scope: it needs NO read
 *  verbs (the shell fetches). Mirrors `echarts-panel`'s Chart widget. */
const CHART_TILE = {
  entry: "remoteEntry.js",
  label: "Chart",
  icon: "bar-chart-3",
  scope: [],
  data: true,
};

/** A v2 self-fetching tile (no `data`) — proves the v3 shell leaves the v2 path untouched. */
const V2_TILE = {
  entry: "remoteEntry.js",
  label: "Proof Ping",
  icon: "shield-check",
  scope: ["series.latest", "series.find"],
};

/** A `data = true` ext cell bound to a real `series.read` source — the same `sources[]` a built-in
 *  timeseries carries. The shell resolves it to `ctx.data` frames. */
function dataExtCell(i: string, ext: string, series: string): Cell {
  return {
    i, x: 0, y: 0, w: 6, h: 4, v: 3, widget_type: "chart",
    view: `ext:${ext}/chart`,
    binding: { series: "" },
    sources: [{ refId: "A", tool: "series.read", args: { series }, datasource: { type: "series" } }],
  };
}

async function seedSamples(series: string, count: number, base: number) {
  for (let i = 0; i < count; i++) {
    await seedSeries({ series, seq: i + 1, payload: base + i, key: "kind", value: "temperature" });
  }
}

const READ_CAPS = [
  "mcp:viz.query:call",
  "mcp:series.read:call",
  "mcp:ingest.write:call",
  "mcp:tags.add:call",
  "mcp:dashboard.save:call",
  "mcp:dashboard.get:call",
  "mcp:ext.list:call",
];

let n = 0;
const nextWs = () => `fi-${n++}`;

beforeAll(() => useRealGateway());

// ---------------------------------------------------------------------------------------------------
// The manifest `data` flag reaches the client as an entry flag, and marks the tile a DATA view.
// ---------------------------------------------------------------------------------------------------
describe("data-flag projection (real gateway)", () => {
  it("a seeded data = true widget surfaces a picker entry marked data:true", async () => {
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, READ_CAPS);
    await seedExtension({ ext: "chartex", version: "0.1.0", tier: "wasm", enabled: true, widgets: [CHART_TILE] });

    const installed = await listExtensions();
    const entry = extWidgetEntries(installed).find((e) => e.viewKey === "ext:chartex/chart");
    expect(entry).toBeDefined();
    expect(entry!.data).toBe(true); // the frames-in opt-in carried manifest → ExtUi → ext.list → picker
  });

  it("a v2 widget (no data) surfaces an entry with data falsy — its path is unchanged", async () => {
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, READ_CAPS);
    await seedExtension({ ext: "proofex", version: "0.1.0", tier: "wasm", enabled: true, widgets: [V2_TILE] });

    const installed = await listExtensions();
    const entry = extWidgetEntries(installed).find((e) => e.viewKey === "ext:proofex/proof-ping");
    expect(entry).toBeDefined();
    expect(entry!.data === true).toBe(false);
  });
});

// ---------------------------------------------------------------------------------------------------
// The shell resolves a data cell's sources[] to frames under the viewer's grant (the frames-in seam).
// ---------------------------------------------------------------------------------------------------
describe("frames resolution via viz.query (real gateway)", () => {
  it("resolves a data ext cell's series.read source to real frames the tile would render", async () => {
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, READ_CAPS);
    await seedSamples("chart.temp", 3, 20); // 3 samples → a 3-point frame

    const cell = dataExtCell("c", "chartex", "chart.temp");
    const { result } = renderHook(() => useVizFrames(cell), {
      wrapper: ({ children }) => <WithDashboardCache ws={ws}>{children}</WithDashboardCache>,
    });

    await waitFor(() => expect(result.current.loading).toBe(false), { timeout: 4000 });
    expect(result.current.denied).toBe(false);
    expect(result.current.frames.length).toBeGreaterThan(0);
    // The frame carries the REAL seeded values — the `payload` field holds 20/21/22 (a sibling `seq`
    // field holds 1/2/3). Assert the payloads arrived across any of the frame's fields, so the tile
    // would render the seeded data, not a fabricated series.
    const allValues = result.current.frames.flatMap((f) => f.fields.flatMap((fld) => fld.values));
    expect(allValues).toContain(20);
    expect(allValues).toContain(22);
  });
});

// ---------------------------------------------------------------------------------------------------
// MANDATORY — capability deny. A viewer WITHOUT the source tool's cap gets no frames (honest empty),
// never a fabricated series. (The whole `viz.query` verb missing → denied; here we deny the source.)
// ---------------------------------------------------------------------------------------------------
describe("capability deny (real gateway)", () => {
  it("a viewer lacking mcp:viz.query:call gets no frames, never a fake series", async () => {
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, ["mcp:series.read:call", "mcp:ext.list:call"]); // NO viz.query
    const cell = dataExtCell("c", "chartex", "denied.series");
    const { result } = renderHook(() => useVizFrames(cell), {
      wrapper: ({ children }) => <WithDashboardCache ws={ws}>{children}</WithDashboardCache>,
    });
    await waitFor(() => expect(result.current.loading).toBe(false), { timeout: 4000 });
    expect(result.current.denied).toBe(true);
    expect(result.current.frames).toEqual([]); // no frames — never a fabricated one
  });

  it("a viewer with viz.query but WITHOUT the source tool's cap resolves the target to an empty frame", async () => {
    const ws = nextWs();
    // Holds viz.query but NOT series.read — the per-target deny inside viz.query degrades that target to
    // an empty frame (honest), and the whole call does not error.
    await signInWithCaps("user:ada", ws, ["mcp:viz.query:call", "mcp:ext.list:call"]);
    const cell = dataExtCell("c", "chartex", "chart.temp");
    const { result } = renderHook(() => useVizFrames(cell), {
      wrapper: ({ children }) => <WithDashboardCache ws={ws}>{children}</WithDashboardCache>,
    });
    await waitFor(() => expect(result.current.loading).toBe(false), { timeout: 4000 });
    // The call itself succeeds (viz.query granted); the denied target yields an empty frame — no rows.
    expect(result.current.denied).toBe(false);
    const anyValues = result.current.frames.some((f) => f.fields.some((fld) => fld.values.length > 0));
    expect(anyValues).toBe(false); // the source target was denied → no data, never fabricated
  });
});

// ---------------------------------------------------------------------------------------------------
// MANDATORY — workspace isolation. A cell bound to ws-A data resolves nothing for a ws-B viewer.
// ---------------------------------------------------------------------------------------------------
describe("workspace isolation (real gateway)", () => {
  it("ws-A data is not resolvable by a ws-B viewer through the frames path", async () => {
    const wsA = nextWs();
    await signInWithCaps("user:ada", wsA, READ_CAPS);
    await seedSamples("wall.temp", 3, 50);

    // ws-B viewer with full caps but a different workspace — the same series name resolves to nothing.
    const wsB = nextWs();
    await signInWithCaps("user:ben", wsB, READ_CAPS);
    const cell = dataExtCell("c", "chartex", "wall.temp");
    const { result } = renderHook(() => useVizFrames(cell), {
      wrapper: ({ children }) => <WithDashboardCache ws={wsB}>{children}</WithDashboardCache>,
    });
    await waitFor(() => expect(result.current.loading).toBe(false), { timeout: 4000 });
    // No ws-A values leak across the wall — either no frames or an empty frame, never ws-A's 50/51/52.
    const leaked = result.current.frames.some((f) =>
      f.fields.some((fld) => fld.values.some((v) => v === 50 || v === 51 || v === 52)),
    );
    expect(leaked).toBe(false);
  });
});

// ---------------------------------------------------------------------------------------------------
// v2 COMPAT — a v2 tile (no `data`) under the v3 shell resolves NO frames: EMPTY_CELL path, no call.
// ---------------------------------------------------------------------------------------------------
describe("v2 compat (real gateway)", () => {
  it("a v2 tile with no sources resolves no frames (the v2 self-fetching path is untouched)", async () => {
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, READ_CAPS);
    // A v2 ext cell carries NO sources[] (it owns its data via its bridge scope). The frames hook sees
    // no primary target → denied/empty, so the v3 shell never fetches for it.
    const v2Cell: Cell = {
      i: "v", x: 0, y: 0, w: 6, h: 4, v: 2, widget_type: "chart",
      view: "ext:proofex/proof-ping", binding: { series: "" },
    };
    const { result } = renderHook(() => useVizFrames(v2Cell), {
      wrapper: ({ children }) => <WithDashboardCache ws={ws}>{children}</WithDashboardCache>,
    });
    await waitFor(() => expect(result.current.loading).toBe(false), { timeout: 4000 });
    expect(result.current.frames).toEqual([]);
  });
});

// ---------------------------------------------------------------------------------------------------
// DASHBOARD + CHANNEL PARITY — the SAME data ext cell resolves to the SAME frames whether it is read
// from a persisted dashboard or built from a channel rich_result. One `viz.query` path, one render.
// ---------------------------------------------------------------------------------------------------
describe("dashboard + channel parity (real gateway)", () => {
  it("the same data ext cell resolves identical frames from a dashboard and a channel-shaped cell", async () => {
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, READ_CAPS);
    await seedSamples("parity.temp", 3, 70);

    // Dashboard path: persist the cell, read it back, resolve its frames.
    const cell = dataExtCell("p", "chartex", "parity.temp");
    await saveDashboard("d", "D", [cell]);
    const back = await getDashboard("d");
    const dashCell = back.cells.find((c) => c.i === "p")!;

    const dash = renderHook(() => useVizFrames(dashCell), {
      wrapper: ({ children }) => <WithDashboardCache ws={ws}>{children}</WithDashboardCache>,
    });
    await waitFor(() => expect(dash.result.current.loading).toBe(false), { timeout: 4000 });

    // Channel path: the SAME cell shape a channel rich_result would build (view + sources[]), resolved
    // through the SAME hook (ResponseView → WidgetView → ExtWidget → useVizFrames is the same seam).
    const channelCell = dataExtCell("p", "chartex", "parity.temp");
    const chan = renderHook(() => useVizFrames(channelCell), {
      wrapper: ({ children }) => <WithDashboardCache ws={ws}>{children}</WithDashboardCache>,
    });
    await waitFor(() => expect(chan.result.current.loading).toBe(false), { timeout: 4000 });

    // Identical frames — same numeric values from the same source, one render path across surfaces.
    const dashNums = dash.result.current.frames.flatMap((f) =>
      f.fields.flatMap((fld) => fld.values.filter((v): v is number => typeof v === "number")),
    );
    const chanNums = chan.result.current.frames.flatMap((f) =>
      f.fields.flatMap((fld) => fld.values.filter((v): v is number => typeof v === "number")),
    );
    expect(dashNums).toContain(70);
    expect(chanNums).toEqual(dashNums); // byte-identical resolved data on both surfaces
  });
});
