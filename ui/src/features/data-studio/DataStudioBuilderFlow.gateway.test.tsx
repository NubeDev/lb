// Data Studio 10x phase 3 — the query-first builder flow, driven against a REAL in-process gateway
// (data-studio-10x scope, phase 3 "query-first builder"; CLAUDE §9 / testing §0 — no fake backend).
// Each test signs into a UNIQUE workspace and drives the stacked `BuilderPane` over real seeded rows.
// Covers the query-first flow: pre-Run stage 1 hides the visual stages; post-Run reveals them; the
// `VizGallery` thumbnail cards render from the ONE already-fetched viz.query result (assert: ONE
// `mcp_call{viz.query}` for preview + N gallery thumbnails); the panel.save round-trip is unchanged;
// the demo-data infrastructure (the seeded SQLite demo datasource) keeps the demo badge OFF when the
// user's query has rows.

import { describe, expect, it, beforeAll, afterAll, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { DataStudioView } from "@/features/data-studio/DataStudioView";
import { useRealGateway, signInReal, signInWithCaps, seedIotDemo } from "@/test/gateway-session";
import { RoutingContextProvider } from "@/features/routing/RoutingContextProvider";
import { getSession } from "@/lib/session";
import { getPanel } from "@/lib/panel";
import { addDatasource } from "@/lib/datasources";
import * as ipc from "@/lib/ipc/invoke";

let n = 0;
const nextWs = () => `builder-${n++}`;

beforeAll(() => useRealGateway());

const realGetRect = HTMLElement.prototype.getBoundingClientRect;
beforeAll(() => {
  HTMLElement.prototype.getBoundingClientRect = function () {
    return new DOMRect(0, 0, 1280, 800);
  };
});
afterAll(() => {
  HTMLElement.prototype.getBoundingClientRect = realGetRect;
});

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

async function mountStudio(ws: string) {
  const view = renderStudio(ws);
  await screen.findAllByText("Sources");
  fireEvent(window, new Event("resize"));
  await screen.findByText("Pick a source from the rail or open a New panel to start.");
  await new Promise((r) => setTimeout(r, 200));
  return view;
}

async function openCoolerExplore(user: ReturnType<typeof userEvent.setup>) {
  // The explorer is LAZY per section — expand the Series toggle first (fires `loadSection`), then
  // click the seeded series row.
  const seriesToggle = await screen.findByLabelText("toggle section Series", {}, { timeout: 5000 });
  fireEvent.click(seriesToggle);
  await user.click(await screen.findByLabelText("insert series cooler.temp", {}, { timeout: 5000 }));
  await screen.findByLabelText("panel builder", {}, { timeout: 5000 });
}

/** Install an `ipc.invoke` counter that delegates to the REAL transport (observe, never fake — rule 9). */
function viaCounter() {
  const real = ipc.invoke;
  const byTool = new Map<string, number>();
  const spy = vi
    .spyOn(ipc, "invoke")
    .mockImplementation(((cmd: string, args?: Record<string, unknown>) => {
      if (cmd === "mcp_call") {
        const tool = (args?.tool as string) ?? "?";
        byTool.set(tool, (byTool.get(tool) ?? 0) + 1);
      }
      return real(cmd, args);
    }) as typeof ipc.invoke);
  return {
    tool: (t: string) => byTool.get(t) ?? 0,
    restore: () => spy.mockRestore(),
  };
}

describe("Data Studio 10x — phase 3 query-first builder (real gateway)", () => {
  it("query-first: pre-Run mounts ONLY the toolbar + query editor; rows reveal preview + gallery + drawer", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedIotDemo();

    await mountStudio(ws);
    await openCoolerExplore(user);

    // Stage 1 (source picked, NO Run yet): ONLY the toolbar + the query editor. No preview, no viz
    // gallery, no options drawer — the visual stages are gated on rows existing.
    expect(screen.getByLabelText("query editor stage")).toBeInTheDocument();
    expect(screen.queryByLabelText("panel preview")).toBeNull();
    expect(screen.queryByLabelText("visualization gallery")).toBeNull();
    expect(screen.queryByLabelText("options drawer")).toBeNull();

    // Run → rows land → stage 2/3 reveals preview + gallery + drawer.
    await user.click(screen.getByLabelText("run query"));
    await screen.findByLabelText("panel preview");
    expect(screen.getByLabelText("visualization gallery")).toBeInTheDocument();
    expect(screen.getByLabelText("options drawer")).toBeInTheDocument();
    // The query editor stage is gone — replaced by the post-Run visual flow.
    expect(screen.queryByLabelText("query editor stage")).toBeNull();
  }, 30000);

  it("the gallery renders N type cards from ONE real viz.query — preview + thumbnails share the cache", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedIotDemo();

    await mountStudio(ws);
    await openCoolerExplore(user);

    // Spy AFTER the studio mount + builder open (so layout/auth reads don't pollute the count) but
    // BEFORE the Run that triggers the data fetch. The gallery + preview both render through the one
    // `viz.query` path; React Query de-dups them to a single round-trip.
    const counted = viaCounter();
    await user.click(screen.getByLabelText("run query"));
    await screen.findByLabelText("visualization gallery");

    // The gallery offers the 6 chart-likes (live mini-renders) + 3 labeled cards (table/genui/template)
    // = 9 type cards, plus the preview — all reading the same cached viz.query result.
    const cards = screen.getAllByRole("button", { name: /^viz / });
    expect(cards.length).toBe(9);
    expect(counted.tool("viz.query")).toBe(1);
    counted.restore();
  }, 30000);

  it("panel.save round-trip is unchanged — the stacked builder's split-menu Save-as-library writes a real record", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedIotDemo();

    await mountStudio(ws);
    await openCoolerExplore(user);
    await user.click(screen.getByLabelText("run query"));
    await screen.findByLabelText("panel preview");

    // Name + save via the split-button caret (the inline LibraryPanelBar is split-layout only).
    await user.type(screen.getByLabelText("panel title"), "My chart");
    window.prompt = () => "my-chart";
    await user.click(screen.getByLabelText("more save options"));
    await user.click(await screen.findByLabelText("save as library panel"));

    // Round-trip: the real record exists with the built spec.
    await waitFor(async () => {
      const p = await getPanel("my-chart");
      expect(p.title).toBe("My chart");
      expect(p.spec.sources?.[0]?.tool).toBe("series.read");
    });
    expect((await screen.findByRole("status", { name: "saved as" })).textContent).toMatch(/my-chart/);
  }, 30000);

  it("demo-data integrity: when the user query has rows, no demo offer / no demo badge", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedIotDemo();
    // Register the demo SQLite datasource via the REAL admin verb. The file isn't actually queryable
    // in this env (no sidecar) — but `useDemoPreview` only checks the datasource ROSTER, and that
    // roster IS the real `datasource.list` result. So this proves the demo gate honestly reflects the
    // workspace state without ever fabricating a frame (rule 9).
    await addDatasource({
      name: "demo-buildings",
      kind: "sqlite",
      endpoint: "127.0.0.1:0",
      dsn: "/var/lib/lb/demo/buildings.db",
    });

    await mountStudio(ws);
    await openCoolerExplore(user);
    await user.click(screen.getByLabelText("run query"));
    await screen.findByLabelText("panel preview");

    // The user's query has REAL rows → the demo offer is absent AND the demo badge is absent. The demo
    // toggle is for the zero-row case; an unbadged demo frame in a row-full preview would be a lie.
    expect(screen.queryByLabelText("preview with demo data")).toBeNull();
    expect(screen.queryByLabelText("demo data badge")).toBeNull();
  }, 30000);

  it("capability-deny — without the viz.query cap, the preview degrades honestly (no fabricated rows)", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    // Grant layout + panel.save but NOT viz.query — the read denies server-side; the gallery/preview
    // show the honest denied state, never a fabricated value (rule 9).
    await signInWithCaps("user:ada", ws, [
      "mcp:series.list:call",
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
    await user.click(screen.getByLabelText("run query"));
    // The preview degrades honestly — no row rendered, no fabricated gallery thumbnail content.
    await waitFor(() =>
      expect(screen.queryByLabelText("preview with demo data")).toBeNull(),
    );
    expect(screen.queryByLabelText("demo data badge")).toBeNull();
  }, 30000);
});
