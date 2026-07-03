// The Dashboards page, driven against a REAL in-process gateway (dashboard scope; CLAUDE §9 / testing
// §0 — no fake backend). Each test logs in to a UNIQUE workspace, seeds real, tagged series through
// the real ingest path, and drives the real `DashboardView` + hook + api client + HTTP transport.
// Covers: the data-studio-v2 REMOVAL REGRESSION (the dashboard has NO panel-authoring surface — no
// "Add panel", no per-cell edit; it PLACES library panels and renders); create → place a library
// panel → it renders + persists; seeded cells render with the full option surface; variables; rename;
// delete; workspace isolation. Panel AUTHORING is covered by the Data Studio gateway tests.
// (The per-verb capability deny + gate-3 membership deny are proven server-side in the Rust tests.)

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { useState } from "react";

import { DashboardView } from "./DashboardView";
import { saveDashboard } from "@/lib/dashboard";
import { savePanel } from "@/lib/panel";
import { cellToSpec } from "@/lib/panel";
import type { Cell } from "@/lib/dashboard";
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

/** A built v3 timeseries cell bound to the seeded `cooler.temp` series — what a Data Studio builder
 *  tab produces. Seeded through the REAL write paths (`panel.save` / `dashboard.save`), per rule 9. */
function builtSeriesCell(i = "w1", title?: string): Cell {
  return {
    i,
    x: 0,
    y: 0,
    w: 8,
    h: 4,
    v: 3,
    widget_type: "chart",
    view: "timeseries",
    binding: { series: "" },
    ...(title ? { title } : {}),
    sources: [{ refId: "A", tool: "series.read", args: { series: "cooler.temp" }, datasource: { type: "surreal" } }],
  };
}

/** Create a dashboard titled `title` in the freshly-rendered view (it auto-selects on create). */
async function createDashboard(user: ReturnType<typeof userEvent.setup>, title: string) {
  await user.type(await screen.findByLabelText("new dashboard title"), title);
  await user.click(screen.getByLabelText("create dashboard"));
}

describe("DashboardView (real gateway)", () => {
  it("REMOVAL REGRESSION: no authoring on the dashboard — but a library panel places and renders", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedIotDemo();

    // A library panel authored in Data Studio (the REAL `panel.save` write path, rule 9).
    await savePanel("cooler-temp", "Cooler temp", cellToSpec(builtSeriesCell("spec")));

    renderDashboard(ws);
    await createDashboard(user, "Ops");

    // The authoring surface is GONE from the dashboard (data-studio scope v2): no "Add panel", the
    // panel factory lives at /t/$ws/data-studio now. "Add library panel" (placement) remains.
    expect(screen.queryByLabelText("add panel")).toBeNull();

    // Place the library panel: the ref-cell flow, the dashboard's only way to gain a panel.
    await user.click(await screen.findByLabelText("add library panel"));
    await user.click(await screen.findByRole("option", { name: /Cooler temp/ }));
    await screen.findByLabelText("cell w1");

    // NO per-cell edit affordance on the placed cell (authoring removed; geometry/remove remain).
    expect(screen.queryByLabelText("edit cell w1")).toBeNull();

    // Persisted + hydrated: a fresh render re-loads the dashboard, the host hydrates the ref cell on
    // `dashboard.get`, and it renders the timeseries over real rows (the SVG line + latest).
    renderDashboard(ws);
    await user.click(await screen.findByLabelText("select dashboard ops"));
    await screen.findByLabelText("cell w1");
    expect(await screen.findByLabelText("timeseries line")).toBeInTheDocument();
    expect((await screen.findByLabelText("timeseries latest")).textContent).not.toBe("");
  });

  it("renders a timeseries panel over a bridged source with the full option surface", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedIotDemo();

    // The full option surface is authored in Data Studio now; the dashboard renders it. Seed a built
    // cell with a fieldConfig unit through the REAL `dashboard.save` write path (rule 9).
    const cell = builtSeriesCell("w1");
    cell.fieldConfig = { defaults: { unit: "celsius" }, overrides: [] };
    await saveDashboard("tagged", "Tagged", [cell]);

    renderDashboard(ws);
    await user.click(await screen.findByLabelText("select dashboard tagged"));

    await screen.findByLabelText("cell w1");
    // The latest value renders a real (numeric) value formatted through the bridge — not a fake.
    expect((await screen.findByLabelText("timeseries latest")).textContent).not.toBe("");
  });

  it("a studio-edited cell (title change through dashboard.save) renders + persists", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedIotDemo();

    // Editing happens in Data Studio (BuilderPane, covered there); the dashboard consumes the saved
    // record. Seed → rename through the REAL write path → the dashboard shows the edit.
    await saveDashboard("cfg", "Cfg", [builtSeriesCell("w1")]);
    await saveDashboard("cfg", "Cfg", [builtSeriesCell("w1", "Web01 CPU")]);

    renderDashboard(ws);
    await user.click(await screen.findByLabelText("select dashboard cfg"));
    // The title renders in the cell header AND (as the series displayName) the legend — ≥1 is the point.
    expect((await screen.findAllByText("Web01 CPU")).length).toBeGreaterThan(0);
    expect(await screen.findByLabelText("timeseries line")).toBeInTheDocument();
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

    // The panel resolves `${host}` → `cooler.temp` and renders real rows read through the bridge. The
    // v2 `chart` view aliases to the v3 `timeseries` renderer (canonicalView), so the labels are
    // `timeseries *` — proving a shipped v2 cell renders through the new path unchanged.
    await screen.findByLabelText("cell w1");
    expect(await screen.findByLabelText("timeseries line")).toBeInTheDocument();
    expect((await screen.findByLabelText("timeseries latest")).textContent).not.toBe("");
  });

  it("renames a dashboard from the roster (title-only save, layout preserved) and it persists", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedIotDemo();

    // Give it a real cell so we can prove the rename preserves the layout (title-only save must not
    // blank the cells). Seeded through the real write path (authoring lives in Data Studio now).
    await saveDashboard("ops", "Ops", [builtSeriesCell("w1")]);

    renderDashboard(ws);
    await user.click(await screen.findByLabelText("select dashboard ops"));
    await screen.findByLabelText("cell w1");

    // Rename inline from the roster: pencil → edit field → new title → confirm.
    await user.click(await screen.findByLabelText("rename dashboard ops"));
    const field = await screen.findByLabelText("rename dashboard ops");
    await user.clear(field);
    await user.type(field, "Operations");
    await user.click(screen.getByLabelText("confirm rename ops"));

    // The roster row now shows the new title (same id `ops`, title changed). The title renders in more
    // than one place (roster row + header), so assert AT LEAST one — `findByText` throws on multiples.
    expect((await screen.findAllByText("Operations")).length).toBeGreaterThan(0);

    // Persisted + layout preserved: reload, reselect, the cell is still there under the new title. The
    // reload mounts a second view into the same document (the prior one isn't unmounted), so "Operations"
    // appears multiple times — assert presence, not uniqueness.
    renderDashboard(ws);
    await user.click((await screen.findAllByLabelText("select dashboard ops"))[0]);
    expect((await screen.findAllByText("Operations")).length).toBeGreaterThan(0);
    expect((await screen.findAllByLabelText("cell w1")).length).toBeGreaterThan(0);
  });

  it("deletes a dashboard from the roster through the confirm gate; it disappears from the list", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);

    renderDashboard(ws);
    await createDashboard(user, "Doomed");
    expect(await screen.findByLabelText("select dashboard doomed")).toBeInTheDocument();

    // Trash icon → the shared destructive confirm → Delete.
    await user.click(await screen.findByLabelText("delete dashboard doomed"));
    await user.click(await screen.findByLabelText("confirm action"));

    // Gone from the roster (real tombstone via `dashboard.delete`; `dashboard.list` no longer returns it).
    expect(await screen.findByText("No dashboards yet.")).toBeInTheDocument();
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
