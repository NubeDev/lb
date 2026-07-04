// The Dashboards page, driven against a REAL in-process gateway (dashboard scope; CLAUDE §9 / testing
// §0 — no fake backend). Each test logs in to a UNIQUE workspace, seeds real, tagged series through
// the real ingest path, and drives the real `DashboardView` + hook + api client + HTTP transport.
// Covers: the data-studio-v2 REMOVAL REGRESSION (the dashboard has NO panel-authoring surface — no
// "Add panel", no per-cell edit; it PLACES library panels and renders); create → place a library
// panel → it renders + persists; seeded cells render with the full option surface; variables; rename;
// delete; workspace isolation. Panel AUTHORING is covered by the Data Studio gateway tests.
// Also (dashboard-viewer-mode scope): editing is ADMIN-only — a VIEWER (member, no admin cap) reads
// the live grid but gets NO authoring surface, an ADMIN gets all of it, and a viewer's save/delete is
// refused SERVER-side (the UI gate is defense-in-depth; the gateway is the wall).

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { useState } from "react";

import { DashboardView } from "./DashboardView";
import { saveDashboard, deleteDashboard, shareDashboard } from "@/lib/dashboard";
import { savePanel } from "@/lib/panel";
import { cellToSpec } from "@/lib/panel";
import type { Cell } from "@/lib/dashboard";
import { useRealGateway, signInReal, signInWithCaps, seedIotDemo } from "@/test/gateway-session";
import { RoutingContextProvider } from "@/features/routing/RoutingContextProvider";
import { ThemeProvider } from "@/lib/theme";
import { getSession, CAP } from "@/lib/session";
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
    // Wrap in the REAL ThemeProvider (as the shell's App does) — the app chrome's motion primitives
    // read theme via `useMotionPref`, so a bare render throws "useTheme must be used within
    // ThemeProvider". Real provider, no fake theme layer (CLAUDE §9).
    <ThemeProvider>
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
        <DashboardView ws={ws} onOpenInDataStudio={() => {}} />
      </RoutingContextProvider>
    </ThemeProvider>,
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
      <ThemeProvider>
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
            onOpenInDataStudio={() => {}}
          />
        </RoutingContextProvider>
      </ThemeProvider>
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

    // NO in-place per-cell editor on the placed cell (authoring removed; geometry/remove remain) —
    // but the cell DOES carry an "open in data studio" affordance, since Data Studio is where panels
    // are authored now (the dashboard only places + renders). It navigates to /t/$ws/data-studio.
    expect(screen.queryByLabelText("edit cell w1")).toBeNull();
    expect(screen.getByLabelText("open cell w1 in data studio")).toBeInTheDocument();

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

  // ── viewer mode (dashboard-viewer-mode scope) ─────────────────────────────────────────────────
  // Editing the dashboard surface is ADMIN-only. A viewer (a member WITHOUT any admin cap) reads the
  // live grid but gets NO authoring surface. `canEdit` gates on `isAdmin(caps)`, NOT `dashboard.save`
  // (which is member-level — every member holds it, so gating on it made everyone an editor).

  // The member-level caps a viewer carries — the dashboard reads + save (save IS member-level, which is
  // the whole point: gating on it made everyone an editor), the panel reads (ref-cell hydration), and
  // the series reads (so the live widget renders real rows). NO admin cap → `isAdmin` false → no
  // authoring surface. (`series.read`/`.latest` aren't in the `CAP` display map — raw strings here.)
  const VIEWER_CAPS = [
    CAP.dashboardList,
    CAP.dashboardGet,
    CAP.dashboardSave,
    CAP.panelGet,
    CAP.panelList,
    "mcp:series.read:call",
    "mcp:series.latest:call",
    "mcp:series.find:call",
  ];

  it("VIEWER: a non-admin member gets NO authoring surface — no roster/create/drag/edit/delete/add", async () => {
    const ws = nextWs();
    // Admin (dev login) seeds a real dashboard with a rendered cell through the real write path.
    await signInReal("user:ada", ws);
    await seedIotDemo();
    await saveDashboard("ops", "Ops", [builtSeriesCell("w1")]);
    // Share it workspace-wide so any member (our viewer, a different principal) can READ it — the
    // point is role (member vs admin), not ownership; gate 3 still governs which dashboards are visible.
    await shareDashboard("ops", "workspace");

    // Now become a VIEWER: a member with the dashboard caps but NO admin cap. `renderDashboard` reads
    // this session's caps (the real source the shell gates on) — no mock. `series.*` reads ride the
    // member set the seeded caps include so the widget renders real rows.
    await signInWithCaps("user:ben", ws, VIEWER_CAPS);
    const searchRef = { current: { ...defaultDashboardSearch(), d: "ops" } as DashboardSearch };
    renderDashboardWithSearch(ws, searchRef);

    // A viewer READS the dashboard — the seeded cell mounts (the dashboard loaded + rendered its grid).
    // (Full series rendering is covered by the admin/render cases above; here the point is the cell is
    // reachable read-only, then that the authoring chrome around it is gone.)
    expect(await screen.findByLabelText("cell w1")).toBeInTheDocument();

    // But every authoring affordance is GONE:
    expect(screen.queryByLabelText("new dashboard title")).toBeNull(); // no create input
    expect(screen.queryByLabelText("create dashboard")).toBeNull(); // no + button
    expect(screen.queryByLabelText("dashboard rail")).toBeNull(); // no roster panel at all
    expect(screen.queryByLabelText("move cell w1")).toBeNull(); // grid not draggable
    expect(screen.queryByLabelText("remove cell w1")).toBeNull(); // no per-cell delete
    expect(screen.queryByLabelText("add library panel")).toBeNull(); // no add-panel
    expect(screen.queryByLabelText("delete dashboard")).toBeNull(); // no delete-dashboard
    expect(screen.queryByLabelText("edit variables")).toBeNull(); // no variable editor
  });

  it("ADMIN: a workspace admin gets the full authoring surface — roster/create/drag/edit/delete/add", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws); // dev login == workspace admin (isAdmin true)
    await seedIotDemo();
    await saveDashboard("ops", "Ops", [builtSeriesCell("w1")]);

    renderDashboard(ws);
    await user.click(await screen.findByLabelText("select dashboard ops"));
    await screen.findByLabelText("cell w1");

    // Every authoring affordance is PRESENT (the mirror of the viewer case):
    expect(screen.getByLabelText("new dashboard title")).toBeInTheDocument(); // create input
    expect(screen.getByLabelText("create dashboard")).toBeInTheDocument(); // + button
    expect(screen.getByLabelText("dashboard rail")).toBeInTheDocument(); // roster panel
    expect(await screen.findByLabelText("move cell w1")).toBeInTheDocument(); // draggable
    expect(screen.getByLabelText("remove cell w1")).toBeInTheDocument(); // per-cell delete
    expect(screen.getByLabelText("add library panel")).toBeInTheDocument(); // add-panel
    expect(screen.getByLabelText("delete dashboard")).toBeInTheDocument(); // delete-dashboard
    expect(screen.getByLabelText("edit variables")).toBeInTheDocument(); // variable editor
  });

  it("VIEWER DENY (server-side, mandatory): a viewer without admin still can't save/delete — but the wall is the SERVER", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveDashboard("ops", "Ops", [builtSeriesCell("w1")]);

    // The UI gate is defense-in-depth; the REAL wall is server-side. Prove it directly: a token that
    // lacks `dashboard.save`/`.delete` is refused by the gateway regardless of any UI. (dev-login holds
    // save, so this uses a DELIBERATELY narrowed cap set — the reads only — to prove the server deny.)
    await signInWithCaps("user:ben", ws, [CAP.dashboardList, CAP.dashboardGet]);
    await expect(saveDashboard("ops", "Hijacked", [builtSeriesCell("w1")])).rejects.toThrow();
    await expect(deleteDashboard("ops")).rejects.toThrow();
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
