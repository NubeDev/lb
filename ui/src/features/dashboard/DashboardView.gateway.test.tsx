// The Dashboards page, driven against a REAL in-process gateway (dashboard scope; CLAUDE §9 / testing
// §0 — no fake backend). Each test logs in to a UNIQUE workspace, seeds real, tagged series through
// the real ingest path, and drives the real `DashboardView` + hook + api client + HTTP transport.
// Covers: create → select → add a widget bound to a real series → it renders + persists; a tag-bound
// widget resolves via `series.find`; and workspace isolation (a fresh workspace shows no dashboards).
// (The per-verb capability deny + gate-3 membership deny are proven server-side in the Rust tests;
// the nav cap-gating is unit-tested separately.)

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { useState } from "react";

import { DashboardView } from "./DashboardView";
import { saveDashboard } from "@/lib/dashboard";
import { useRealGateway, signInReal, seedIotDemo } from "@/test/gateway-session";
import { RoutingContextProvider } from "@/features/routing/RoutingContextProvider";
import { getSession } from "@/lib/session";
import {
  defaultDashboardSearch,
  varsFromSearch,
  type DashboardSearch,
} from "@/features/routing/search";

let n = 0;
const nextWs = () => `dash-ui-${n++}`;

beforeAll(() => useRealGateway());

/** Render `DashboardView` inside the shell's routing context, fed the REAL signed session's caps (the
 *  same source the live shell uses to gate the edit affordance — no mock, the caps are real). The dev
 *  login carries `mcp:dashboard.save:call`, so the builder is shown; a deny case would pass fewer caps. */
function renderDashboard(ws: string) {
  const s = getSession();
  return render(
    <RoutingContextProvider
      value={{
        workspace: ws,
        principal: s?.principal ?? "",
        caps: s?.caps,
        allowed: ["dashboards"],
        extPages: [],
        extPagesLoading: false,
        onSignOut: () => {},
        switchWorkspace: () => {},
      }}
    >
      <DashboardView ws={ws} />
    </RoutingContextProvider>,
  );
}

/** A harness that holds the dashboard search in state (standing in for the router's navigate) so a test
 *  can assert the variable selection round-trips to the URL search. Exposes the live search via a ref. */
function renderDashboardWithSearch(ws: string, searchRef: { current: DashboardSearch }) {
  const s = getSession();
  function Harness() {
    const [search, setSearch] = useState<DashboardSearch>(searchRef.current);
    searchRef.current = search;
    return (
      <RoutingContextProvider
        value={{
          workspace: ws,
          principal: s?.principal ?? "",
          caps: s?.caps,
          allowed: ["dashboards"],
          extPages: [],
          extPagesLoading: false,
          onSignOut: () => {},
          switchWorkspace: () => {},
        }}
      >
        <DashboardView
          ws={ws}
          range={search}
          onSearchChange={(next) => {
            searchRef.current = next;
            setSearch(next);
          }}
        />
      </RoutingContextProvider>
    );
  }
  return render(<Harness />);
}

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

    renderDashboard(ws);
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
    renderDashboard(ws);
    await user.click(await screen.findByLabelText("select dashboard ops"));
    expect(await screen.findByLabelText("cell w1")).toBeInTheDocument();
  });

  it("renders a stat view over a bridged source", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedIotDemo();

    renderDashboard(ws);
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

  it("Slice 1 — ⚙ settings: add → rename + change view → save → reload re-renders with edits", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedIotDemo();

    renderDashboard(ws);
    await createDashboard(user, "Cfg");

    // Add a chart bound to the seeded series.
    await screen.findByRole("option", { name: "cooler.temp" });
    await user.selectOptions(await screen.findByLabelText("widget source"), "series:cooler.temp");
    await user.click(screen.getByLabelText("add widget"));
    await screen.findByLabelText("cell w1");

    // Open the ⚙ settings drawer for the cell, rename it + switch the view to stat, save. Scope queries
    // to the drawer (the add-builder also has a title/view field — the drawer is the `widget settings`
    // region).
    await user.click(screen.getByLabelText("settings cell w1"));
    const drawer = within(await screen.findByLabelText("widget settings"));
    const titleField = await drawer.findByLabelText("widget title");
    await user.clear(titleField);
    await user.type(titleField, "Web01 CPU");
    // The drawer seeded the source; switch the view to stat and save.
    await user.selectOptions(drawer.getByLabelText("widget view"), "stat");
    await user.click(drawer.getByLabelText("save widget"));

    // The renamed title shows in the header (the derived label is replaced).
    expect(await screen.findByText("Web01 CPU")).toBeInTheDocument();

    // Persisted: reload re-renders the cell with the new title + view (stat value, not a chart line).
    renderDashboard(ws);
    await user.click(await screen.findByLabelText("select dashboard cfg"));
    expect(await screen.findByText("Web01 CPU")).toBeInTheDocument();
    expect((await screen.findByLabelText("stat value")).textContent).not.toBe("");
  });

  it("Slice 2 — define a variable, it persists on the record, its selection syncs to the URL + reloads", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);

    const searchRef = { current: defaultDashboardSearch() };
    renderDashboardWithSearch(ws, searchRef);
    await createDashboard(user, "Vars");

    // Open the variable editor, add a custom multi/include-all variable `env` (deterministic options —
    // the query-over-bridge resolution path is unit-tested in resolveOptions + store_query_test).
    await user.click(await screen.findByLabelText("edit variables"));
    const editor = within(await screen.findByLabelText("variable editor"));
    await user.click(editor.getByLabelText("add variable"));
    await user.clear(editor.getByLabelText("variable name"));
    await user.type(editor.getByLabelText("variable name"), "env");
    await user.type(editor.getByLabelText("variable custom values"), "prod, staging");
    await user.click(editor.getByLabelText("save variables"));

    // The variable bar now shows an `env` dropdown with the real options; selecting `prod` syncs to the
    // URL search (`?var-env=prod`) — selection lives in the URL (per-viewer, shareable).
    const bar = within(await screen.findByLabelText("variable bar"));
    const dropdown = await bar.findByLabelText("variable env");
    await bar.findByRole("option", { name: "prod" });
    await user.selectOptions(dropdown, "prod");
    expect(varsFromSearch(searchRef.current)).toEqual({ env: "prod" });

    // The DEFINITION persisted on the record: a fresh render re-loads it and the bar reappears.
    renderDashboardWithSearch(ws, searchRef);
    await user.click(await screen.findByLabelText("select dashboard vars"));
    const bar2 = within(await screen.findByLabelText("variable bar"));
    expect(await bar2.findByLabelText("variable env")).toBeInTheDocument();
  });

  it("Slice 3 — a cell source interpolates a variable: ${host} → the selected series renders real rows", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedIotDemo(); // real `cooler.temp` rows

    // Seed (via the real write path) a dashboard with a `host` custom variable and a chart cell whose
    // source templates `series` with `${host}` — the interpolation payoff. Selecting host=cooler.temp in
    // the URL must make the chart read `cooler.temp` through the bridge.
    await saveDashboard(
      "interp",
      "Interp",
      [
        {
          i: "w1",
          x: 0,
          y: 0,
          w: 4,
          h: 3,
          v: 2,
          widget_type: "chart",
          view: "chart",
          binding: { series: "" },
          source: { tool: "series.read", args: { series: "${host}" } },
        },
      ],
      [{ name: "host", type: "custom", custom: ["cooler.temp", "fryer.state"] }],
    );

    // Render with host=cooler.temp selected in the URL search.
    const searchRef = { current: { ...defaultDashboardSearch(), "var-host": "cooler.temp" } as DashboardSearch };
    renderDashboardWithSearch(ws, searchRef);
    await user.click(await screen.findByLabelText("select dashboard interp"));

    // The chart resolves `${host}` → `cooler.temp` and renders real rows read through the bridge.
    await screen.findByLabelText("cell w1");
    expect(await screen.findByLabelText("chart line")).toBeInTheDocument();
    expect((await screen.findByLabelText("chart latest")).textContent).not.toBe("");
  });

  it("is workspace isolated — a fresh workspace shows no dashboards", async () => {
    const user = userEvent.setup();

    // Ada creates a dashboard in her workspace.
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    renderDashboard(wsA);
    await createDashboard(user, "Ops A");
    expect(await screen.findByLabelText("select dashboard ops-a")).toBeInTheDocument();

    // Ben, in a different workspace, sees an empty roster (the hard wall).
    const wsB = nextWs();
    await signInReal("user:ben", wsB);
    renderDashboard(wsB);
    expect(await screen.findByText("No dashboards yet.")).toBeInTheDocument();
  });
});
