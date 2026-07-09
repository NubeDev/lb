// The `weather` built-in view, against a REAL spawned node (CLAUDE §9, testing §0). The node's
// `weather.current` fetch is repointed (at spawn, in `real-gateway.ts`) at a real local HTTP stub
// serving a canned Open-Meteo body — no mocked client, the one sanctioned external fake-boundary.
//
// Covers: WeatherPanel renders the seeded reading through `usePanelData` → `viz.query` →
// `weather.current` (the same path every built-in view uses); the mandatory capability-deny (a
// missing `mcp:weather.current:call` cap → honest denied, never a fabricated reading); and workspace
// isolation (a ws-B weather dashboard is invisible to ws-A).

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

import { useRealGateway, signInReal, signInWithCaps } from "@/test/gateway-session";
import { saveDashboard, getDashboard } from "@/lib/dashboard/dashboard.api";
import type { Cell } from "@/lib/dashboard";
import { WeatherPanel } from "./WeatherPanel";
import { WithDashboardCache } from "@/features/dashboard/cache/testCacheWrapper";

let n = 0;
const nextWs = () => `weather-${n++}`;

beforeAll(() => useRealGateway());

/** A weather cell over the `weather.current` target — a fixed lat/lon (the seeded stub ignores it). */
function weatherCell(i: string): Cell {
  return {
    i, x: 0, y: 0, w: 6, h: 4, v: 3, widget_type: "chart", view: "weather",
    binding: { series: "" },
    sources: [
      {
        refId: "A",
        tool: "weather.current",
        args: { lat: -27.47, lon: 153.02 },
        datasource: { type: "surreal" },
      },
    ],
  };
}

const WEATHER_CAPS = ["mcp:viz.query:call", "mcp:weather.current:call"];

describe("WeatherPanel (real gateway)", () => {
  it("renders the seeded reading through usePanelData → viz.query → weather.current", async () => {
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, [
      ...WEATHER_CAPS,
      "mcp:dashboard.save:call",
      "mcp:dashboard.get:call",
    ]);

    const cell = weatherCell("w");
    await saveDashboard("d", "D", [cell]);
    const back = await getDashboard("d");

    render(
      <WithDashboardCache ws={ws}>
        <WeatherPanel cell={back.cells.find((c) => c.i === "w")!} label="Weather" />
      </WithDashboardCache>,
    );

    await waitFor(() => expect(screen.getByLabelText("weather temp").textContent).toContain("21.4"), {
      timeout: 4000,
    });
    expect(screen.getByLabelText("weather condition").textContent).toBe("Overcast");
    expect(screen.getByLabelText("weather wind").textContent).toContain("11.2");
    expect(screen.getByLabelText("weather updated").textContent).toContain("2026-07-09 12:00");
  });
});

describe("weather.current capability-deny (real gateway)", () => {
  it("without the weather.current cap the target degrades to an honest empty — never a fabricated reading", async () => {
    // `viz.query`'s per-target denial degrades to an honest empty frame rather than surfacing as the
    // panel's top-level `denied` state (viz/query.rs: "a denied target … degrades to an honest empty
    // frame, never a fabrication") — so the panel shows its no-data state, not a number.
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, ["mcp:viz.query:call"]); // NO mcp:weather.current:call
    const cell = weatherCell("w");

    render(
      <WithDashboardCache ws={ws}>
        <WeatherPanel cell={cell} label="Weather" />
      </WithDashboardCache>,
    );

    await waitFor(() => expect(screen.getByRole("status").textContent).toMatch(/no value/i), {
      timeout: 4000,
    });
    expect(screen.queryByLabelText("weather temp")).not.toBeInTheDocument();
  });

  it("without the viz.query cap itself the panel is denied, never a fabricated reading", async () => {
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, ["mcp:weather.current:call"]); // NO mcp:viz.query:call
    const cell = weatherCell("w");

    render(
      <WithDashboardCache ws={ws}>
        <WeatherPanel cell={cell} label="Weather" />
      </WithDashboardCache>,
    );

    await waitFor(() => expect(screen.getByRole("status").textContent).toMatch(/no access/i), {
      timeout: 4000,
    });
    expect(screen.queryByLabelText("weather temp")).not.toBeInTheDocument();
  });
});

describe("workspace isolation (real gateway)", () => {
  it("a ws-B weather dashboard is invisible to ws-A", async () => {
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await saveDashboard("shared-weather", "A", [weatherCell("w")]);

    const wsB = nextWs();
    await signInReal("user:ben", wsB);
    await expect(getDashboard("shared-weather")).rejects.toThrow();
  });
});
