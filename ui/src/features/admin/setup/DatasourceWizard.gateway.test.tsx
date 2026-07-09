// The Data → insight wizard, driven against a REAL in-process seeded gateway (setup scope; CLAUDE §9 —
// no fake backend). Proves the wizard is pure orchestration over the real verbs: registering the demo
// datasource lands a real `datasource.list` row; saving the panel lands a real `dashboard.save` record
// (read back over the gateway, with the timeseries cell bound to the federation query); running the
// rule drives the real `rules.run`; and the insights step mounts the real read widget. A fresh
// workspace per test isolates the shared node.
//
// Note: no federation sidecar is spawned in this env (same as the Query-workbench gateway test), so the
// buildings query returns no rows here — we assert the REAL WRITE effects (the source row, the saved
// dashboard) and that the run/insights paths mount + complete honestly, never a fabricated result.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DashboardCacheProvider } from "@/features/dashboard/cache/DashboardQueryProvider";

import { DatasourceWizard } from "./DatasourceWizard";
import { DEFAULT_SOURCE } from "./dataToInsight";
import { CAP } from "@/lib/session/admin-caps";
import { listDatasources } from "@/lib/datasources";
import { getDashboard, listDashboards } from "@/lib/dashboard";
import { useRealGateway, signInWithCaps } from "@/test/gateway-session";

// The caps the wizard's steps drive: datasource read/add, query, dashboard save, rule run, insight list.
const CAPS = [
  CAP.datasourceList,
  "mcp:datasource.add:call",
  "mcp:federation.query:call",
  CAP.dashboardSave,
  CAP.dashboardGet,
  CAP.dashboardList,
  CAP.rulesRun,
  CAP.insightList,
];

let n = 0;
const nextWs = () => `ds-wiz-${n++}`;

// Mount inside the SAME `DashboardCacheProvider` the SetupHub wraps the wizard in — its query cache +
// `useDashboardWs` context are what `useDatasourceList` and the live `WidgetHost` preview read through.
function renderWizard(ws: string) {
  return render(
    <DashboardCacheProvider ws={ws}>
      <DatasourceWizard ws={ws} caps={CAPS} />
    </DashboardCacheProvider>,
  );
}

beforeAll(() => useRealGateway());

describe("DatasourceWizard (real seeded gateway)", () => {
  it("registers a real datasource, saves a real dashboard, and mounts the run + insights paths", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, CAPS);
    renderWizard(ws);

    // ── Step 1 (intro): the overview names the six parts. ──
    await screen.findByText("From a datasource to a live insight");
    expect(screen.getByText(/Datasource — where the data lives/)).toBeInTheDocument();
    expect(screen.getByText(/Insight — the durable finding/)).toBeInTheDocument();
    await user.click(screen.getByLabelText("Continue"));

    // ── Step 2 (datasource): register the demo → a REAL datasource.list row lands. ──
    await user.click(await screen.findByLabelText("Register the buildings demo datasource"));
    await waitFor(async () => {
      const rows = await listDatasources();
      expect(rows.some((d) => d.name === DEFAULT_SOURCE)).toBe(true);
    });
    await user.click(screen.getByLabelText("Continue"));

    // ── Step 3 (SQL): the query is preloaded; Run drives the real engine (empty here, no sidecar). ──
    await screen.findByLabelText("query");
    await user.click(screen.getByLabelText("Run the query"));
    await waitFor(() => expect(screen.getByLabelText("Run the query")).toBeEnabled());
    await user.click(screen.getByLabelText("Continue"));

    // ── Step 4 (panel + dashboard): Save lands a REAL dashboard with the timeseries cell. ──
    await screen.findByLabelText("panel preview");
    await user.click(screen.getByLabelText("Create the dashboard"));
    await screen.findByText(/Saved to dashboard/i);

    // Read the saved dashboard back over the gateway — the cell is a timeseries bound to the query.
    const saved = await waitFor(async () => {
      const list = await listDashboards();
      const row = list.find((d) => d.title === "Energy by site");
      expect(row).toBeTruthy();
      return row!;
    });
    const dash = await getDashboard(saved.id);
    expect(dash.cells).toHaveLength(1);
    expect(dash.cells[0].view).toBe("timeseries");
    expect(dash.cells[0].sources?.[0].tool).toBe("federation.query");
    // The panel splits ONE LINE PER SITE — the plot spec's `seriesField` pivots the long frame. Without
    // this the renderer collapses every site into a single line (the reported bug).
    const plot = (dash.cells[0].options as { plot?: { seriesField?: string; xField?: string } }).plot;
    expect(plot?.seriesField).toBe("site");
    expect(plot?.xField).toBe("hour");
    await user.click(screen.getByLabelText("Continue"));

    // ── Step 5 (rule): the rule is preloaded read-only; Run drives the real rules.run + completes. ──
    await screen.findByLabelText("rule");
    await user.click(screen.getByLabelText("Run the rule"));
    await waitFor(() => expect(screen.getByLabelText("Run the rule")).toBeEnabled());
    await user.click(screen.getByLabelText("Continue"));

    // ── Step 6 (insights): the real read widget mounts (no crash; deduped-count note shown). ──
    await screen.findByText(/deduped by its/);
  });
});
