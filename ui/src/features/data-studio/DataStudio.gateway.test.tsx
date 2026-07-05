// Data Studio 10x — the Dockview multi-pane workbench, driven against a REAL in-process gateway
// (data-studio-10x scope, all 4 phases; CLAUDE §9 / testing §0 — no fake backend). Each test signs
// into a UNIQUE workspace and drives the real view + Dockview + api clients + HTTP transport. Covers
// the headline (pick a seeded source via the CatalogExplorer tree → a BUILDER TAB renders real rows
// through `viz.query` → Save-as-library `panel.save` round-trips through the new Save split-button),
// the SQL-editor-when-needed (surfaced in the stacked Query section only after a source that needs it
// is picked — the picker in the rail's Sources tab is the CatalogExplorer now), opening an existing
// panel from the Library into the stacked builder, the per-user LAYOUT PERSISTENCE (the real
// `layout.get`/`set` verbs — a reload restores the tabs + drafts; another member sees THEIR OWN
// default), the legacy-layout fallback (a stored flexlayout blob → default workbench + reset notice),
// the mandatory capability-deny (no `panel.save` → no "save as library panel" split-menu item + the
// verb denied server-side) and workspace isolation (nothing crosses to ws-B).

import { describe, expect, it, beforeAll, afterAll } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { DataStudioView } from "./DataStudioView";
import { DATA_STUDIO_SURFACE } from "./workbenchModel";
import { useRealGateway, signInReal, signInWithCaps, seedIotDemo } from "@/test/gateway-session";
import { RoutingContextProvider } from "@/features/routing/RoutingContextProvider";
import { getSession } from "@/lib/session";
import { getPanel, listPanels, savePanel } from "@/lib/panel";
import { getLayout, setLayout } from "@/lib/layout";

let n = 0;
const nextWs = () => `studio-${n++}`;

beforeAll(() => useRealGateway());

// jsdom computes no layout. Dockview also measures DOM (its split panes call `getBoundingClientRect`),
// so give every element a real-sized rect — same stub the FlexLayout tests used (rect-stubbing carries
// over per the scope's testing plan). Restored after the file — the gateway pool shares a worker.
const realGetRect = HTMLElement.prototype.getBoundingClientRect;
beforeAll(() => {
  HTMLElement.prototype.getBoundingClientRect = function () {
    return new DOMRect(0, 0, 1280, 800);
  };
});
afterAll(() => {
  HTMLElement.prototype.getBoundingClientRect = realGetRect;
});

/** Render the studio inside the shell's routing context, fed the REAL signed session's caps. */
function renderStudio(ws: string, allowed: string[] = ["data-studio"]) {
  const s = getSession();
  return render(
    <RoutingContextProvider
      value={{
        workspace: ws,
        principal: s?.principal ?? "",
        caps: s?.caps,
        allowed: allowed as never,
        extPages: [],
        extPagesLoading: false,
        onSignOut: () => {},
        switchWorkspace: () => {},
      }}
    >
      <DataStudioView ws={ws} />
    </RoutingContextProvider>,
  );
}

/** Render + wait for the dock to mount (the saved layout loads async), then fire a resize so
 *  Dockview measures its (stubbed) rect and draws the panel contents. */
async function mountStudio(ws: string, allowed = ["data-studio"]) {
  const view = renderStudio(ws, allowed);
  // The Sources/Library rail tab buttons render once the studio shell mounts.
  await screen.findAllByText("Sources");
  fireEvent(window, new Event("resize"));
  return view;
}

/** Click the seeded `cooler.temp` series row in the CatalogExplorer tree → opens a BUILDER tab. */
async function openCoolerExplore(user: ReturnType<typeof userEvent.setup>) {
  // The rail's Sources tab is now a CatalogExplorer tree (system-catalog scope). Clicking the seeded
  // series row yields a `series.read` source → the studio opens a stacked builder tab on it.
  const seriesRow = await screen.findByLabelText("insert series cooler.temp");
  await user.click(seriesRow);
  await screen.findByLabelText("panel builder");
}

/** Run the staged query (the stacked builder reveals preview/gallery/options only after rows exist). */
async function runStagedQuery(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByLabelText("run query"));
  await screen.findByLabelText("panel preview");
}

describe("Data Studio 10x — the Dockview workbench (real gateway)", () => {
  it("pick source → stacked builder → save as library panel round-trips; the layout + draft persist per user", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedIotDemo();

    const first = await mountStudio(ws);
    await openCoolerExplore(user);

    // Stacked query-first: PRE-RUN, only the toolbar + the query editor mount (no preview/gallery/options).
    expect(screen.queryByLabelText("panel preview")).toBeNull();
    expect(screen.queryByLabelText("visualization gallery")).toBeNull();

    // Run the seeded query → rows land → the visual stages reveal (preview on top, gallery below, options
    // folded into the collapsed drawer). All through the ONE render path (`PreviewPane` → `WidgetView` →
    // `viz.query`).
    await runStagedQuery(user);

    // Name it and save it to the library. The stacked builder's Save split-button's caret reveals the
    // "save as library panel" menu item (the inline LibraryPanelBar is split-layout only).
    const title = screen.getByLabelText("panel title");
    await user.clear(title);
    await user.type(title, "Cooler explore");
    window.prompt = () => "cooler-explore";
    await user.click(screen.getByLabelText("more save options"));
    await user.click(await screen.findByLabelText("save as library panel"));

    // Round-trip: the REAL record exists with the built spec (source + view), per rule 9.
    await waitFor(async () => {
      const p = await getPanel("cooler-explore");
      expect(p.title).toBe("Cooler explore");
      expect(p.spec.sources?.[0]?.tool).toBe("series.read");
    });
    // The tab shows the compact saved-as badge (still role="status", named lookup).
    expect((await screen.findByRole("status", { name: "saved as" })).textContent).toMatch(/cooler-explore/);

    // LAYOUT PERSISTENCE: the debounced `layout.set` lands the model (incl. the tabs) in the caller's
    // member-owned record (versioned `{engine:"dockview", model}`).
    await waitFor(
      async () => {
        const l = await getLayout(DATA_STUDIO_SURFACE);
        expect(JSON.stringify(l.model)).toContain("builder");
        expect(JSON.stringify(l.model)).toContain("cooler.temp");
      },
      { timeout: 8000 },
    );

    // Reload: a fresh mount restores the debugging setup — the dock tab's title + the builder surface.
    first.unmount();
    await mountStudio(ws);
    expect((await screen.findAllByText("cooler.temp")).length).toBeGreaterThan(0);
    expect(await screen.findByLabelText("panel builder")).toBeInTheDocument();
  }, 30000);

  it("the layout record is MEMBER-OWNED — another member gets their own default workbench", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedIotDemo();

    const ada = await mountStudio(ws);
    await openCoolerExplore(user);
    await waitFor(
      async () => expect(JSON.stringify((await getLayout(DATA_STUDIO_SURFACE)).model)).toContain("builder"),
      { timeout: 8000 },
    );
    ada.unmount();

    // Ben, SAME workspace: his own (absent) layout — the default workbench, no builder tab. (Minted via
    // the seed-session route — dev login only auto-provisions a FRESH workspace.)
    await signInWithCaps("user:ben", ws, [
      "mcp:series.list:call",
      "mcp:series.read:call",
      "mcp:viz.query:call",
      "mcp:layout.get:call",
      "mcp:layout.set:call",
    ]);
    const l = await getLayout(DATA_STUDIO_SURFACE);
    expect(l.model).toBeNull();
    await mountStudio(ws);
    // The CatalogExplorer renders (the rail's default Sources tab); no builder tab yet.
    await screen.findByLabelText("insert series cooler.temp");
    expect(screen.queryByLabelText("panel builder")).toBeNull();
  }, 30000);

  it("workspace isolation — the layout and the saved panel never cross to ws-B", async () => {
    const user = userEvent.setup();
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await seedIotDemo();

    const a = await mountStudio(wsA);
    await openCoolerExplore(user);
    window.prompt = () => "walled-panel";
    await user.click(screen.getByLabelText("more save options"));
    await user.click(await screen.findByLabelText("save as library panel"));
    await waitFor(async () => expect((await listPanels()).length).toBe(1));
    await waitFor(
      async () => expect(JSON.stringify((await getLayout(DATA_STUDIO_SURFACE)).model)).toContain("builder"),
      { timeout: 8000 },
    );
    a.unmount();

    // The hard wall: same user, different workspace — no layout, no panels.
    const wsB = nextWs();
    await signInReal("user:ada", wsB);
    expect((await getLayout(DATA_STUDIO_SURFACE)).model).toBeNull();
    expect(await listPanels()).toEqual([]);
  }, 30000);

  it("capability-deny — no `panel.save`: no split-menu save-as-library, and the verb is refused server-side", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    // Enough to explore (list/read series through viz.query) + persist a layout — but NOT panel.save.
    await signInWithCaps("user:cap", ws, [
      "mcp:series.list:call",
      "mcp:series.read:call",
      "mcp:viz.query:call",
      "mcp:layout.get:call",
      "mcp:layout.set:call",
    ]);
    // Seed real rows under a full session, then drop back to the capped one.
    const capped = getSession();
    await signInReal("user:ada", ws);
    await seedIotDemo();
    const { setSession } = await import("@/lib/session");
    setSession(capped!);

    await mountStudio(ws);
    await openCoolerExplore(user);

    // Without the cap, the split-button caret ("more save options") is absent — no save-as-library
    // affordance (the primary Save-to-tab stays; it persists to the in-memory draft only).
    expect(screen.queryByLabelText("more save options")).toBeNull();
    expect(screen.queryByLabelText("save as library panel")).toBeNull();
    // …and the host is the real boundary regardless.
    await expect(
      savePanel("sneak", "Sneak", { widget_type: "chart", binding: { series: "" } }),
    ).rejects.toThrow();
  }, 30000);

  it("SQL editor surfaces for a Direct-SurrealDB source and is absent for a series source (stacked)", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedIotDemo();

    await mountStudio(ws);

    // A series source → the friendly picker, NO SQL editor (the conditional stays hidden).
    await openCoolerExplore(user);
    // Pre-Run, the stage-1 QueryTargets owns the `panel datasource` SELECT + the SQL editor surface.
    expect(screen.queryByLabelText("sql query editor")).toBeNull();

    // Pick "SurrealDB (native)" in the builder tab's Query section, then the "SQL query (direct
    // SurrealDB)" source entry → the Builder⇄Code `SqlQueryEditor` appears (surfaced in the stacked
    // layout, not rebuilt). `panel source` is the SourceCombobox — focus opens it, mouseDown picks.
    const ds = screen.getByLabelText("panel datasource") as HTMLSelectElement;
    await user.selectOptions(ds, "surreal");
    // The select patches the editor state; let the QueryTab re-render with the surreal target before
    // driving the source combobox (a focus before the new paint lands can no-op).
    await new Promise((r) => setTimeout(r, 300));
    fireEvent.focus(await screen.findByLabelText("panel source"));
    const sqlOpt = await screen.findByRole("option", { name: "SQL query (direct SurrealDB)" });
    fireEvent.mouseDown(sqlOpt);
    expect(await screen.findByLabelText("sql query editor", {}, { timeout: 5000 })).toBeInTheDocument();
  }, 30000);

  it("opening an existing library panel lands in the stacked builder (preview + query, one tab)", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedIotDemo();

    // Seed a real library panel to open.
    await savePanel("existing-chart", "Existing chart", {
      title: "Existing chart",
      view: "timeseries",
      sources: [{ refId: "A", tool: "series.read", args: { series: "cooler.temp" } }],
    } as never);

    await mountStudio(ws);
    // Sources is the rail's default tab; switch to the Library rail tab to mount the roster.
    await user.click(await screen.findByRole("tab", { name: "library tab" }));
    // Open it from the Library roster → ONE stacked builder tab (preview on top, query on bottom).
    await user.click(await screen.findByLabelText("open library panel Existing chart"));
    expect(await screen.findByLabelText("panel builder")).toBeInTheDocument();
    // Stacked builder requires a Run to reveal preview; run it, then assert preview renders.
    await runStagedQuery(user);
    expect(await screen.findByLabelText("panel preview")).toBeInTheDocument();
    // The title round-tripped into the editor — the chart is the focus, its source available below.
    await waitFor(() =>
      expect((screen.getByLabelText("panel title") as HTMLInputElement).value).toBe("Existing chart"),
    );
  }, 30000);

  it("the studio rail minimizes to the shared collapsed strip and expands back", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);

    await mountStudio(ws);
    // The CatalogExplorer tree is mounted on the Sources rail tab.
    await screen.findByLabelText("data explorer");

    // Minimize → the rail folds to the shared CollapsedRail strip (same kit as every other surface).
    await user.click(screen.getByLabelText("minimize studio rail"));
    expect(screen.queryByLabelText("data explorer")).toBeNull();
    await user.click(await screen.findByLabelText("expand studio rail"));
    expect(await screen.findByLabelText("data explorer")).toBeInTheDocument();
  }, 30000);

  it("legacy-layout fallback — a stored flexlayout blob → default workbench + the one-time reset notice", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedIotDemo();

    // Pre-seed a LEGACY flexlayout blob (no `engine:"dockview"` tag) as the member's saved layout —
    // exactly what a real returning user from the v2/v3 era has in their record.
    await setLayout(DATA_STUDIO_SURFACE, {
      // A recognizable flexlayout-era shape: `layout` top-level key, no `engine`.
      layout: { id: "root", type: "row", children: [{ type: "tabset", children: [{ type: "tab" }] }] },
    } as never);

    await mountStudio(ws);
    // The dock falls back to the default workbench (no crash) AND surfaces the one-time notice.
    expect(await screen.findByLabelText("layout reset notice")).toBeInTheDocument();
    // The catalog still renders — the studio is usable. The corrupted layout is NOT restored.
    await screen.findByLabelText("data explorer");
    // Dismiss the notice.
    await userEvent.setup().click(screen.getByLabelText("dismiss layout reset notice"));
    await waitFor(() => expect(screen.queryByLabelText("layout reset notice")).toBeNull());
  }, 30000);
});
