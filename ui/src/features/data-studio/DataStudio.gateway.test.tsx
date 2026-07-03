// Data Studio v2 — the multi-pane workbench, driven against a REAL in-process gateway (data-studio
// scope v2, "v2 testing plan"; CLAUDE §9 / testing §0 — no fake backend). Each test signs into a
// UNIQUE workspace and drives the real view + FlexLayout + api clients + HTTP transport. Covers the
// headline (pick a seeded source → an EXPLORE TAB renders real rows through `viz.query` → a BUILDER
// TAB opens from it → Save-as-library `panel.save` round-trips), the per-user LAYOUT PERSISTENCE
// (the real `layout.get`/`set` verbs: a reload restores the tabs + drafts; another member sees THEIR
// OWN default — member-owned), the mandatory capability-deny (no `panel.save` → no save affordance +
// the verb denied server-side), and workspace isolation (nothing crosses to ws-B).

import { describe, expect, it, beforeAll, afterAll } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { DataStudioView } from "./DataStudioView";
import { DATA_STUDIO_SURFACE } from "./workbenchModel";
import { useRealGateway, signInReal, signInWithCaps, seedIotDemo } from "@/test/gateway-session";
import { RoutingContextProvider } from "@/features/routing/RoutingContextProvider";
import { getSession } from "@/lib/session";
import { getPanel, listPanels, savePanel } from "@/lib/panel";
import { getLayout } from "@/lib/layout";

let n = 0;
const nextWs = () => `studio-${n++}`;

beforeAll(() => useRealGateway());

// jsdom computes no layout, and FlexLayout refuses to draw tab content into a 0×0 rect (its
// `updateRect` guards `width !== 0`). Give every element a real-sized rect and nudge the layout via a
// window `resize` (FlexLayout's resize listener calls `updateRect` synchronously). Restored after the
// file — the gateway pool shares a worker across files.
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
function renderStudio(ws: string) {
  const s = getSession();
  return render(
    <RoutingContextProvider
      value={{
        workspace: ws,
        principal: s?.principal ?? "",
        caps: s?.caps,
        allowed: ["data-studio"],
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
 *  FlexLayout measures its (stubbed) rect and draws the tab contents. */
async function mountStudio(ws: string) {
  const view = renderStudio(ws);
  await screen.findAllByText("Sources");
  fireEvent(window, new Event("resize"));
  return view;
}

/** Pick the seeded `cooler.temp` series in the Sources pane → opens an explore tab. */
async function openCoolerExplore(user: ReturnType<typeof userEvent.setup>) {
  const source = await screen.findByLabelText("explore source");
  await screen.findByRole("option", { name: "cooler.temp" });
  await user.selectOptions(source, "series:cooler.temp");
  // The explore tab mounted: its toolbar (view toggle + Build) is live.
  await screen.findByLabelText("build panel from explore");
}

describe("Data Studio v2 workbench (real gateway)", () => {
  it("explore → build → save as library panel round-trips; the layout + draft persist per user", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedIotDemo();

    const first = await mountStudio(ws);
    await openCoolerExplore(user);

    // The explore preview renders through the ONE render path (`WidgetHost` → `viz.query`).
    expect(await screen.findByTestId("explore-preview")).toBeInTheDocument();

    // Build: a BUILDER TAB opens on the explored draft — the full option surface, inline (no modal).
    await user.click(screen.getByLabelText("build panel from explore"));
    expect(await screen.findByLabelText("panel builder")).toBeInTheDocument();

    // Name it and save it to the library (the shipped `panel.save` flow; the slug prompt is stubbed).
    const title = await screen.findByLabelText("panel title");
    await user.clear(title);
    await user.type(title, "Cooler explore");
    window.prompt = () => "cooler-explore";
    await user.click(screen.getByLabelText("save as library panel"));

    // Round-trip: the REAL record exists with the built spec (source + view), per rule 9.
    await waitFor(async () => {
      const p = await getPanel("cooler-explore");
      expect(p.title).toBe("Cooler explore");
      expect(p.spec.sources?.[0]?.tool).toBe("series.read");
    });
    // The tab shows the saved-as marker.
    expect((await screen.findByRole("status")).textContent).toMatch(/cooler-explore/);

    // LAYOUT PERSISTENCE: the debounced `layout.set` lands the model (incl. the tabs) in the caller's
    // member-owned record.
    await waitFor(
      async () => {
        const l = await getLayout(DATA_STUDIO_SURFACE);
        expect(JSON.stringify(l.model)).toContain("builder");
        expect(JSON.stringify(l.model)).toContain("cooler.temp");
      },
      { timeout: 8000 },
    );

    // Reload: a fresh mount restores the debugging setup — the explore tab (by its tab button; an
    // inactive tab's content mounts on demand) AND the active builder tab's full surface.
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
      async () => expect(JSON.stringify((await getLayout(DATA_STUDIO_SURFACE)).model)).toContain("explore"),
      { timeout: 8000 },
    );
    ada.unmount();

    // Ben, SAME workspace: his own (absent) layout — the default workbench, no explore tab. (Minted
    // via the seed-session route — dev login only auto-provisions a FRESH workspace.)
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
    await screen.findByLabelText("explore source");
    expect(screen.queryByLabelText("build panel from explore")).toBeNull();
  }, 30000);

  it("workspace isolation — the layout and the saved panel never cross to ws-B", async () => {
    const user = userEvent.setup();
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await seedIotDemo();

    const a = await mountStudio(wsA);
    await openCoolerExplore(user);
    window.prompt = () => "walled-panel";
    await user.click(screen.getByLabelText("build panel from explore"));
    await screen.findByLabelText("panel builder");
    await user.click(screen.getByLabelText("save as library panel"));
    await waitFor(async () => expect((await listPanels()).length).toBe(1));
    await waitFor(
      async () => expect(JSON.stringify((await getLayout(DATA_STUDIO_SURFACE)).model)).toContain("explore"),
      { timeout: 8000 },
    );
    a.unmount();

    // The hard wall: same user, different workspace — no layout, no panels.
    const wsB = nextWs();
    await signInReal("user:ada", wsB);
    expect((await getLayout(DATA_STUDIO_SURFACE)).model).toBeNull();
    expect(await listPanels()).toEqual([]);
  }, 30000);

  it("capability-deny — no `panel.save`: no save affordance, and the verb is refused server-side", async () => {
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
    await user.click(screen.getByLabelText("build panel from explore"));
    await screen.findByLabelText("panel builder");

    // The affordance is gone (the palette-gate precedent)…
    expect(screen.queryByLabelText("save as library panel")).toBeNull();
    // …and the host is the real boundary regardless.
    await expect(savePanel("sneak", "Sneak", { widget_type: "chart", binding: { series: "" } })).rejects.toThrow();
  }, 30000);
});
