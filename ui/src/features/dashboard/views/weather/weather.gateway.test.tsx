// The `weather` built-in view, against a REAL spawned node (CLAUDE §9, testing §0). The node's
// `weather.current` fetch is repointed (at spawn, in `real-gateway.ts`) at a real local HTTP stub
// serving a canned Open-Meteo body — no mocked client, the one sanctioned external fake-boundary.
//
// The `weather` view is SELF-SOURCED: `usePanelData` builds its `{tool:"weather.current", args:{lat,
// lon}}` from `cell.options` (the location the Options step sets, defaulting to Brisbane) and runs it
// through the plain `useSource` tool path — NOT `viz.query` (there is no datasource/transform pipeline,
// it's one gated read returning one `{temp_c,…}` row). So its ONLY capability dependency is
// `mcp:weather.current:call`; it needs no `mcp:viz.query:call`.
//
// Covers: WeatherPanel renders the seeded reading; the mandatory capability-deny (a missing
// `mcp:weather.current:call` cap → honest "no access", never a fabricated reading); and workspace
// isolation (a ws-B weather dashboard is invisible to ws-A).

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

import { useRealGateway, signInReal, signInWithCaps } from "@/test/gateway-session";
import { saveDashboard, getDashboard } from "@/lib/dashboard/dashboard.api";
import type { Cell } from "@/lib/dashboard";
import { WeatherPanel } from "./WeatherPanel";
import { observedLocal } from "./observedLocal";
import { WithDashboardCache } from "@/features/dashboard/cache/testCacheWrapper";

let n = 0;
const nextWs = () => `weather-${n++}`;

beforeAll(() => useRealGateway());

/** A weather cell as the picker creates it: NO user-picked source — the location rides `options.lat/lon`
 *  and `usePanelData` self-sources `weather.current` from it (the seeded stub ignores the coordinate). */
function weatherCell(i: string): Cell {
  return {
    i, x: 0, y: 0, w: 6, h: 4, v: 3, widget_type: "chart", view: "weather",
    binding: { series: "" },
    options: { lat: -27.47, lon: 153.02 },
  };
}

const WEATHER_CAPS = ["mcp:weather.current:call"];

describe("WeatherPanel (real gateway)", () => {
  it("renders the seeded reading — self-sourced from weather.current (no viz.query)", async () => {
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
    // The stub's `time` is the UTC epoch 1783598400 (2026-07-09T12:00:00Z); the panel renders it in the
    // VIEWER's timezone. Assert against the same local-time formatter so this holds under any runner TZ.
    expect(screen.getByLabelText("weather updated").textContent).toContain(observedLocal(1783598400));
  });
});

describe("weather.current capability-deny (real gateway)", () => {
  it("without the weather.current cap the panel is denied — an honest 'no access', never a fabricated reading", async () => {
    // The self-source runs `weather.current` directly through `useSource`; a denied read surfaces as the
    // panel's honest `denied` state ("no access to this source"), never a number. `weather.current` is
    // the ONLY cap the weather tile needs, so signing in with nothing proves the deny.
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, ["mcp:dashboard.save:call"]); // NO mcp:weather.current:call
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
