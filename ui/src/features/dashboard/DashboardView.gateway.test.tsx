// The Dashboards page, driven against a REAL in-process gateway (dashboard scope; CLAUDE §9 / testing
// §0 — no fake backend). Each test logs in to a UNIQUE workspace, seeds real, tagged series through
// the real ingest path, and drives the real `DashboardView` + hook + api client + HTTP transport.
// Covers: create → select → add a widget bound to a real series → it renders + persists; a tag-bound
// widget resolves via `series.find`; and workspace isolation (a fresh workspace shows no dashboards).
// (The per-verb capability deny + gate-3 membership deny are proven server-side in the Rust tests;
// the nav cap-gating is unit-tested separately.)

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { DashboardView } from "./DashboardView";
import { useRealGateway, signInReal, seedIotDemo } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `dash-ui-${n++}`;

beforeAll(() => useRealGateway());

/** Create a dashboard titled `title` in the freshly-rendered view (it auto-selects on create). */
async function createDashboard(user: ReturnType<typeof userEvent.setup>, title: string) {
  await user.type(await screen.findByLabelText("new dashboard title"), title);
  await user.click(screen.getByLabelText("create dashboard"));
}

describe("DashboardView (real gateway)", () => {
  it("creates a dashboard, adds a chart bound to a real series, and persists it", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedIotDemo();

    render(<DashboardView ws={ws} />);
    await createDashboard(user, "Ops");

    // Bind a chart to the seeded `cooler.temp` series and add it.
    await user.type(await screen.findByLabelText("widget series"), "cooler.temp");
    await user.click(screen.getByLabelText("add widget"));

    // The cell renders the chart over real samples (the SVG line + a latest value). `findBy*` waits
    // for the async backfill (`series.read`) to complete and the widget to leave its loading state.
    await screen.findByLabelText("cell w1");
    expect(await screen.findByLabelText("series cooler.temp line")).toBeInTheDocument();
    expect((await screen.findByLabelText("chart latest")).textContent).not.toBe("");

    // Persisted: a fresh render of the same workspace re-loads the dashboard from the store.
    render(<DashboardView ws={ws} />);
    await user.click(await screen.findByLabelText("select dashboard ops"));
    expect(await screen.findByLabelText("cell w1")).toBeInTheDocument();
  });

  it("resolves a tag-bound widget via series.find", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedIotDemo();

    render(<DashboardView ws={ws} />);
    await createDashboard(user, "Tagged");

    // A stat widget bound by tags (no explicit series) resolves to series:cooler.temp via series.find.
    await user.selectOptions(await screen.findByLabelText("widget type"), "stat");
    await user.type(screen.getByLabelText("widget tags"), "kind:temperature");
    await user.click(screen.getByLabelText("add widget"));

    await screen.findByLabelText("cell w1");
    // The stat value renders a real (numeric) latest value, not a fake (await the find→read chain).
    expect((await screen.findByLabelText("stat value")).textContent).not.toBe("");
  });

  it("is workspace isolated — a fresh workspace shows no dashboards", async () => {
    const user = userEvent.setup();

    // Ada creates a dashboard in her workspace.
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    render(<DashboardView ws={wsA} />);
    await createDashboard(user, "Ops A");
    expect(await screen.findByLabelText("select dashboard ops-a")).toBeInTheDocument();

    // Ben, in a different workspace, sees an empty roster (the hard wall).
    const wsB = nextWs();
    await signInReal("user:ben", wsB);
    render(<DashboardView ws={wsB} />);
    expect(await screen.findByText("No dashboards yet.")).toBeInTheDocument();
  });
});
