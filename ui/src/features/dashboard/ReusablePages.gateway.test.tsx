// Reusable pages, driven against a REAL spawned gateway (reusable-pages scope, "UI"; CLAUDE §9 — no
// fake). Proves, end to end over the real store/caps/gateway:
//   1. a TEMPLATE dashboard (a `required` page-parameter variable) round-trips save→get, and the bare
//      template renders the honest RequiredVarGate — its cells do NOT fire while the parameter is
//      unbound (zero series widgets on screen), then picking a value (the URL binding) loads the grid;
//   2. a NAV `template-group` authored through the real builder expands at `nav.resolve` into one
//      instance link per distinct tag value, each carrying its `?var-<var>=<value>` binding on the
//      SAME dashboard record (instance = binding, never copy) — and round-trips through `nav.get`.
// Every list/write is a real verb re-checked server-side; the nav grants nothing (the lens).

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";

import { DashboardView } from "./DashboardView";
import { NavAdmin } from "@/features/admin/nav/NavAdmin";
import { saveDashboard, getDashboard, type Cell } from "@/lib/dashboard";
import { resolveNav, getNav } from "@/lib/nav";
import { invoke } from "@/lib/ipc/invoke";
import { CAP } from "@/lib/session/admin-caps";
import { useRealGateway, signInReal, seedSeries } from "@/test/gateway-session";
import { RoutingContextProvider } from "@/features/routing/RoutingContextProvider";
import { getSession } from "@/lib/session";
import { defaultDashboardSearch, type DashboardSearch } from "@/features/routing/search";

let n = 0;
const nextWs = () => `rp-ui-${n++}`;

const AUTHOR_CAPS = [CAP.navList, CAP.navGet, CAP.navSave, CAP.navDelete, CAP.navShare, CAP.navResolve];

beforeAll(() => useRealGateway());

/** Seed a real `site`-tagged series through the write path (the harness tags `series:<name>` with
 *  `key:value`), so a distinct `site` value is present in the tag graph for the fan-out to enumerate. */
async function seedSite(value: string) {
  await seedSeries({
    series: `hvac.${value}.temp`,
    seq: 1,
    payload: { value: 21 },
    key: "site",
    value,
  });
}

/** A minimal timeseries cell whose source references the `site` parameter — so an unbound render would
 *  splice a `$site` literal (the footgun the gate prevents). */
function siteCell(): Cell {
  return {
    i: "c1",
    x: 0,
    y: 0,
    w: 4,
    h: 3,
    v: 2,
    widget_type: "chart",
    view: "chart",
    title: "Site temp",
    binding: { series: "" },
    source: { tool: "series.read", args: { series: "hvac.${site}.temp" } },
  };
}

/** Render DashboardView holding its search in state (standing in for the router navigate). */
function renderWithSearch(ws: string, initial: DashboardSearch, ref: { current: DashboardSearch }) {
  const s = getSession();
  ref.current = initial;
  function Harness() {
    const [search, setSearch] = useState<DashboardSearch>(initial);
    ref.current = search;
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
            ref.current = next;
            setSearch(next);
          }}
        />
      </RoutingContextProvider>
    );
  }
  return render(<Harness />);
}

describe("Reusable pages (real gateway)", () => {
  it("a template's required variable round-trips and gates cell firing until bound", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);

    // Seed a TEMPLATE: one required `site` parameter + a cell that reads `hvac.${site}.temp`.
    await saveDashboard("site-overview", "Site Overview", [siteCell()], [
      { name: "site", label: "Site", type: "query", required: true },
    ]);

    // A real series for the bound case so the cell renders (title visible) once the parameter is set.
    await seedSite("plant-1");

    // `required` survives the save→get round-trip (additive serde default; no new verb).
    const got = await getDashboard("site-overview");
    expect(got.variables?.find((v) => v.name === "site")?.required).toBe(true);

    // Open the BARE template (no `?var-site`) → the RequiredVarGate shows; the cell does NOT render
    // (no series widget on screen — the grid is gated before any bridge call).
    const ref = { current: defaultDashboardSearch() };
    const gated = renderWithSearch(ws, { ...defaultDashboardSearch(), d: "site-overview" }, ref);

    expect(await screen.findByTestId("required-var-gate")).toBeInTheDocument();
    // The "template · 1 param" hint is shown; the cell (`cell c1`) is NOT (the grid didn't render).
    expect(screen.getByText(/template · 1 param/)).toBeInTheDocument();
    expect(screen.queryByLabelText("cell c1")).not.toBeInTheDocument();
    gated.unmount();

    // Bind the parameter via the URL selection → the gate clears and the grid (the cell) renders.
    const bound = { ...defaultDashboardSearch(), d: "site-overview", "var-site": "plant-1" };
    renderWithSearch(ws, bound, ref);
    await waitFor(() => expect(screen.getByLabelText("cell c1")).toBeInTheDocument());
    expect(screen.queryByTestId("required-var-gate")).not.toBeInTheDocument();
  });

  it("a template-group authored in the builder fans out one bound instance per tag value", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);

    // A template dashboard + three sites present in the tag graph.
    await saveDashboard("site-overview", "Site Overview", [], [
      { name: "site", label: "Site", type: "query", required: true },
    ]);
    await seedSite("plant-1");
    await seedSite("plant-2");
    await seedSite("plant-3");

    // Author a template-group through the REAL builder: "One dashboard per ⟨value⟩".
    render(<NavAdmin ws={ws} caps={AUTHOR_CAPS} />);
    await user.click(await screen.findByLabelText("New nav"));
    await user.type(screen.getByLabelText("Nav title"), "Operations");
    await user.selectOptions(screen.getByLabelText("Item kind"), "template-group");
    await user.selectOptions(screen.getByLabelText("Template dashboard"), "dashboard:site-overview");
    await user.type(screen.getByLabelText("Template parameter"), "site");
    await user.type(screen.getByLabelText("Fan-out facet key"), "site");
    await user.click(screen.getByLabelText("Add item"));
    await user.click(screen.getByLabelText("Save nav"));
    await waitFor(() => expect(screen.getByText("Saved.")).toBeInTheDocument());

    // The authored template-group round-trips through `nav.get` (one dashboard, one parameter, one src).
    const saved = await getNav("operations");
    const tg = saved.items.find((i) => i.kind === "template-group");
    expect(tg?.dashboard).toBe("dashboard:site-overview");
    expect(tg?.var).toBe("site");
    expect(tg?.facets?.[0].key).toBe("site");

    // Pick the nav, then resolve: the template-group expands to ONE instance per distinct site value —
    // each a link to the SAME dashboard record, bound via `vars` (the instance = binding headline).
    await invoke("mcp_call", { tool: "nav.pref.set", args: { id: "operations", now: 0 } });
    const resolved = await resolveNav();
    const group = resolved.items.find((i) => i.kind === "group");
    expect(group).toBeTruthy();
    const bindings = (group?.items ?? []).map((c) => ({
      dashboard: c.dashboard,
      site: c.vars?.site,
    }));
    // Every child is the one template, bound to a distinct site (order-independent).
    for (const b of bindings) expect(b.dashboard).toBe("dashboard:site-overview");
    expect(bindings.map((b) => b.site).sort()).toEqual(["plant-1", "plant-2", "plant-3"]);
  });
});
