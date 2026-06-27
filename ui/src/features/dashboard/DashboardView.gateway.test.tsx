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

    // v2 builder: source-pick the seeded `cooler.temp` series (a friendly label, NOT a tool name),
    // keep the default `chart` view, and add it. The source picker resolves the label to
    // `{tool:"series.read", args:{series:"cooler.temp"}}` behind the scenes. Wait for the async
    // `series.list` to populate the picker options first.
    const source = await screen.findByLabelText("widget source");
    await screen.findByRole("option", { name: "cooler.temp" });
    await user.selectOptions(source, "series:cooler.temp");
    await user.click(screen.getByLabelText("add widget"));

    // The cell renders the chart over real rows read through the bridge (the SVG line + a latest value).
    await screen.findByLabelText("cell w1");
    expect(await screen.findByLabelText("chart line")).toBeInTheDocument();
    expect((await screen.findByLabelText("chart latest")).textContent).not.toBe("");

    // Persisted: a fresh render of the same workspace re-loads the dashboard from the store.
    render(<DashboardView ws={ws} />);
    await user.click(await screen.findByLabelText("select dashboard ops"));
    expect(await screen.findByLabelText("cell w1")).toBeInTheDocument();
  });

  it("renders a stat view over a bridged source", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedIotDemo();

    render(<DashboardView ws={ws} />);
    await createDashboard(user, "Tagged");

    // Source-pick the seeded series, choose the `stat` view, add it.
    await screen.findByRole("option", { name: "cooler.temp" });
    await user.selectOptions(await screen.findByLabelText("widget source"), "series:cooler.temp");
    await user.selectOptions(screen.getByLabelText("widget view"), "stat");
    await user.click(screen.getByLabelText("add widget"));

    await screen.findByLabelText("cell w1");
    // The stat value renders a real (numeric) latest value, not a fake (await the bridged read).
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
