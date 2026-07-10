// PanelPicker unit coverage (reports scope): the two sections a report author picks from.
//   1. Starter widgets — the shared demo cells (`timeseriesCell` + the template gallery minus "ai")
//      render as choices and onPick fires with a renderable Cell (view bound, federation source).
//   2. Library hydrate — clicking a roster row calls getPanel → specToCell → onPick with the hydrated
//      cell (client-side hydration, no save+reload round-trip — the demo-pass addition).
// jsdom has no node, so the `@/lib/panel` transport (`listPanels`/`getPanel`) is mocked; the cell
// builders + specToCell run for real (they're pure factories, not node behavior — §9).

import { describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

// Mock only the transport seams. The cell builders (timeseriesCell/templateCell), specToCell, the
// gallery, and DEMO_SQL/DEFAULT_SOURCE are the real pure factories — exercised for real here.
vi.mock("@/lib/panel", async (orig) => {
  const actual = await orig<typeof import("@/lib/panel")>();
  return {
    ...actual,
    listPanels: vi.fn(),
    getPanel: vi.fn(),
  };
});

import { PanelPicker } from "./PanelPicker";
import { getPanel, listPanels } from "@/lib/panel";
import type { Cell } from "@/lib/dashboard";

afterEach(cleanup);

describe("PanelPicker — starter widgets (the demo cells)", () => {
  it("lists the timeseries + template starter widgets (gallery minus 'ai')", async () => {
    vi.mocked(listPanels).mockResolvedValue([]);
    render(<PanelPicker ws="acme" onPick={() => {}} onCancel={() => {}} />);
    // The timeseries starter.
    expect(await screen.findByText("Energy over time (demo)")).toBeInTheDocument();
    // The three template starters (leader/stats/ranking — the 'ai' scaffold is excluded).
    expect(screen.getByText("Top consumer spotlight (demo)")).toBeInTheDocument();
    expect(screen.getByText("Energy stat tiles (demo)")).toBeInTheDocument();
    expect(screen.getByText("Bar-meter ranking (demo)")).toBeInTheDocument();
    // The draft-with-AI scaffold is filtered out.
    expect(screen.queryByText("Draft with AI (demo)")).not.toBeInTheDocument();
  });

  it("clicking a starter widget fires onPick with a renderable cell (view + federation source)", async () => {
    vi.mocked(listPanels).mockResolvedValue([]);
    const onPick = vi.fn();
    render(<PanelPicker ws="acme" onPick={onPick} onCancel={() => {}} />);
    await screen.findByText("Energy over time (demo)");
    await userEvent.click(screen.getByText("Energy over time (demo)"));
    expect(onPick).toHaveBeenCalledTimes(1);
    const cell: Cell = onPick.mock.calls[0]![0];
    expect(cell.view).toBe("timeseries");
    expect(cell.title).toBe("Energy over time");
    // Bound to the demo-buildings federation source (the shared cell builder's contract).
    expect(cell.sources?.[0]?.tool).toBe("federation.query");
    expect(cell.sources?.[0]?.args).toEqual({ source: "demo-buildings", sql: expect.any(String) });
  });
});

describe("PanelPicker — library hydrate (client-side getPanel → specToCell)", () => {
  it("clicking a library panel row hydrates it and fires onPick with the rendered cell", async () => {
    vi.mocked(listPanels).mockResolvedValue([
      { id: "p1", title: "Site kWh", view: "timeseries", visibility: "private", updated_ts: 1 },
    ]);
    // getPanel returns a spec; specToCell turns it into a renderable cell keyed by the panel id.
    vi.mocked(getPanel).mockResolvedValue({
      id: "p1",
      spec: {
        view: "timeseries",
        widget_type: "chart",
        binding: { series: "" },
        sources: [{ refId: "A", tool: "federation.query", args: {}, datasource: { type: "federation" } }],
        options: {},
        fieldConfig: { defaults: {}, overrides: [] },
      } as never,
    } as never);

    const onPick = vi.fn();
    render(<PanelPicker ws="acme" onPick={onPick} onCancel={() => {}} />);
    // Wait for the roster row to appear, then click it.
    const row = await screen.findByText("Site kWh");
    await userEvent.click(row);
    await waitFor(() => expect(onPick).toHaveBeenCalledTimes(1));

    // The hydrated cell is keyed by the panel id and keeps the roster title.
    const cell: Cell = onPick.mock.calls[0]![0];
    expect(cell.i).toBe("p1");
    expect(cell.title).toBe("Site kWh");
    expect(cell.view).toBe("timeseries");
    // getPanel was called for the picked id (the client-side hydration).
    expect(getPanel).toHaveBeenCalledWith("p1");
  });
});
