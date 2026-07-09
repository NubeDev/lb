// The Insights panel through the wizard (insights-package-scope) — the new-panel flow for a SOURCELESS
// view. Covers the wizard-specific wiring the insights widget added:
//   - the Source step's "no data source" affordance picks the `insights` view + clears the target, so
//     the wizard's source gate is satisfied without a query binding;
//   - the Chart type step (step 2) carries the READ-ONLY toggle (the user-facing headline choice);
//   - Save persists an `insights` cell (view + `options.insights`) through the SAME `dashboard.save`
//     path every panel uses — the no-drift guarantee holds for a sourceless view too;
//   - workspace isolation: a ws-B insights panel never lands in ws-A (the host re-derives ws from token).
//
// Real gateway, no mocks (CLAUDE §9): a real node persists + reloads the cell.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { useRealGateway, signInReal } from "@/test/gateway-session";
import { getDashboard, saveDashboard } from "@/lib/dashboard/dashboard.api";
import { WithDashboardCache } from "@/features/dashboard/cache/testCacheWrapper";
import { PanelWizard } from "@/features/panel-builder/wizard/PanelWizard";

beforeAll(() => useRealGateway());

let n = 0;
const nextWs = () => `inswiz-${n++}`;

describe("Insights panel via the wizard (real gateway)", () => {
  it("picks the sourceless Insights view, sets it interactive on step 2, and saves an insights cell", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveDashboard("d-ins", "Ops", []);
    const user = userEvent.setup();
    render(
      <WithDashboardCache ws={ws}>
        <PanelWizard ws={ws} dashboardId="d-ins" onExit={() => {}} />
      </WithDashboardCache>,
    );

    // Source step: pick the Insights track — a complete "no data source" choice that advances the
    // wizard straight to step 2 (Chart type) in one click.
    await user.click(await screen.findByLabelText("source track insights"));

    // Step 2 (Chart type) — the insights basics carry the read-only toggle. Turn ON acknowledge.
    await waitFor(() => expect(screen.getByLabelText("wizard insights section")).toBeInTheDocument());
    await user.click(screen.getByLabelText("allow acknowledge"));

    // Walk to Save (Options → Transform).
    await user.click(screen.getByText("Next"));
    await waitFor(() => expect(screen.getByLabelText("wizard options step")).toBeInTheDocument());
    await user.click(screen.getByText("Next"));
    await waitFor(() => expect(screen.getByLabelText("wizard transform step")).toBeInTheDocument());
    await user.click(screen.getByLabelText("save panel"));

    // The persisted cell is an insights view, interactive (readOnly:false).
    await waitFor(async () => {
      const d = await getDashboard("d-ins");
      expect(d.cells.length).toBe(1);
    });
    const cell = (await getDashboard("d-ins")).cells[0]!;
    expect(cell.view).toBe("insights");
    const insOpts = (cell.options as Record<string, unknown>).insights as Record<string, unknown>;
    expect(insOpts.readOnly).toBe(false);
    cleanup();
  });

  it("WORKSPACE ISOLATION: a ws-B insights panel never crosses into ws-A", async () => {
    const wsA = nextWs();
    const wsB = nextWs();
    await signInReal("user:ada", wsA);
    await saveDashboard("d-iso", "A-board", []);
    // ws-B saves an insights panel to the SAME dashboard id — the host re-derives ws from the token.
    await signInReal("user:ada", wsB);
    await saveDashboard("d-iso", "B-board", [
      {
        i: "p", x: 0, y: 0, w: 8, h: 4, v: 3, widget_type: "chart",
        view: "insights", binding: { series: "" }, sources: [],
        options: { insights: { readOnly: true, status: "all", severity: "all", limit: 20, showRefresh: true } },
        fieldConfig: { defaults: {}, overrides: [] },
      },
    ]);
    // ws-A's dashboard is untouched.
    await signInReal("user:ada", wsA);
    const aDash = await getDashboard("d-iso");
    expect(aDash.title).toBe("A-board");
    expect(aDash.cells.length).toBe(0);
  });
});
