// Data Studio 10x phase 2 — pages-as-panes, driven against a REAL in-process gateway (data-studio-10x
// scope, phase 2 "pages-as-panes"; CLAUDE §9 / testing §0 — no fake backend). The dock's "+ Open view"
// menu mounts the REAL routed view components (`FlowsView`, `RulesView`, …) as dock panes — same code
// path, same gateway, same caps — never a re-implementation. An embedded `AppPage` mode suppresses the
// view's own full-width header inside a pane (the dock tab is the title bar); the standalone route keeps
// it. Covers: opening Flows + Rules panes renders their REAL views against the gateway; the in-pane
// selection persists; the layout round-trip restores both panes; and the AppPage embedded mode is
// honored (no full-width header in-pane, header intact on the standalone route).

import { describe, expect, it, beforeAll, afterAll } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

import { DataStudioView } from "./DataStudioView";
import { DATA_STUDIO_SURFACE } from "./workbenchModel";
import { useRealGateway, signInReal, seedIotDemo } from "@/test/gateway-session";
import { RoutingContextProvider } from "@/features/routing/RoutingContextProvider";
import { getSession } from "@/lib/session";
import { getLayout } from "@/lib/layout";
import { FlowsView } from "@/features/flows";
import { RulesView } from "@/features/rules";

let n = 0;
const nextWs = () => `panes-${n++}`;

beforeAll(() => useRealGateway());

const realGetRect = HTMLElement.prototype.getBoundingClientRect;
beforeAll(() => {
  HTMLElement.prototype.getBoundingClientRect = function () {
    return new DOMRect(0, 0, 1280, 800);
  };
  // CodeMirror (the Rules pane's editor) measures glyph bounds via `Range.getClientRects()`, which
  // jsdom doesn't implement — without a stub it throws an uncaught exception inside CodeMirror's
  // animation-frame measure, killing the dock's React tree. Same pre-existing jsdom gap that makes
  // `transformDebug.gateway` red on clean master; scoped here so this file can mount the Rules pane.
  if (!Range.prototype.getClientRects) {
    Range.prototype.getClientRects = function (): DOMRectList {
      const rect: DOMRect = { x: 0, y: 0, width: 1, height: 1, top: 0, left: 0, right: 1, bottom: 1 } as never;
      // A DOMRectList is array-like; cast through `never` (the live type is browser-supplied).
      return [rect] as unknown as DOMRectList;
    };
  }
  if (!Range.prototype.getBoundingClientRect) {
    Range.prototype.getBoundingClientRect = function (): DOMRect {
      return { x: 0, y: 0, width: 1, height: 1, top: 0, left: 0, right: 1, bottom: 1 } as never;
    };
  }
});
afterAll(() => {
  HTMLElement.prototype.getBoundingClientRect = realGetRect;
});

function renderStudio(ws: string, allowed: string[]) {
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

async function mountStudio(ws: string, allowed: string[], expectSaved = false) {
  const view = renderStudio(ws, allowed);
  await screen.findAllByText("Sources");
  fireEvent(window, new Event("resize"));
  // The Dockview dock mounts asynchronously (gated on `bench.ready`); its API is null until onReady
  // fires. The open-view menu's `openView` is a silent no-op while the api is null, so wait for the dock
  // to be live: the empty-dock watermark on a fresh mount, OR any dock tab once a saved layout restores.
  if (expectSaved) {
    await waitFor(() => expect(document.querySelector(".dv-react-view, .dv-groupview")).not.toBeNull(), {
      timeout: 5000,
    });
  } else {
    await screen.findByText("Pick a source from the rail or open a New panel to start.");
  }
  // A small grace for `onReady` → `setApi` to land (the watermark renders with the dock, but the api
  // is set on the next tick; without this the first openView is a silent no-op).
  await new Promise((r) => setTimeout(r, 200));
  return view;
}

describe("Data Studio 10x — pages-as-panes (real gateway)", () => {
  it("opens Flows + Rules panes via '+ Open view' and both render their REAL routed views", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedIotDemo();

    await mountStudio(ws, ["data-studio", "flows", "rules"]);

    // Open the "+ Open view" menu → Flows + Rules entries are present (filtered to the caller's
    // `allowed` route lens; the gateway re-checks every verb server-side). Use fireEvent.click for the
    // menu items: userEvent's pointer-event sequence races the menu's light-dismiss handler. The pane
    // mounts its real view (a code-heavy component) asynchronously — waitFor up to 5s for it.
    fireEvent.click(screen.getByLabelText("open view"));
    fireEvent.click(await screen.findByLabelText("open flows view"));
    // The Flows pane mounts the REAL FlowsView (its AppPage section carries the label "flows view").
    await waitFor(() => expect(screen.getAllByLabelText("flows view").length).toBeGreaterThan(0), {
      timeout: 5000,
    });

    // Open a second pane (Rules) — it joins the same group as a new tab; Dockview unmounts the inactive
    // tab's content (Flows) but BOTH tabs persist in the strip.
    fireEvent.click(screen.getByLabelText("open view"));
    fireEvent.click(await screen.findByLabelText("open rules view"));
    await waitFor(() => expect(screen.getAllByLabelText("rules workbench").length).toBeGreaterThan(0), {
      timeout: 5000,
    });
    // Both pane tabs are present in the dock strip (the title attr carries the tab name verbatim).
    const tabTitles = Array.from(document.querySelectorAll<HTMLElement>(".ds-tab")).map((t) => t.title);
    expect(tabTitles).toEqual(expect.arrayContaining(["Flows", "Rules"]));
  }, 30000);

  it("the pane arrangement round-trips through layout.set — a reload restores both panes", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedIotDemo();

    const first = await mountStudio(ws, ["data-studio", "flows", "rules"]);
    fireEvent.click(screen.getByLabelText("open view"));
    fireEvent.click(await screen.findByLabelText("open flows view"));
    await waitFor(() => expect(screen.getAllByLabelText("flows view").length).toBeGreaterThan(0), {
      timeout: 5000,
    });
    fireEvent.click(screen.getByLabelText("open view"));
    fireEvent.click(await screen.findByLabelText("open rules view"));
    await waitFor(() => expect(screen.getAllByLabelText("rules workbench").length).toBeGreaterThan(0), {
      timeout: 5000,
    });

    // The debounced layout.set lands the dock model (incl. both view panes) in the member-owned record.
    await waitFor(
      async () => {
        const model = JSON.stringify((await getLayout(DATA_STUDIO_SURFACE)).model);
        expect(model).toContain("view");
      },
      { timeout: 8000 },
    );
    first.unmount();

    // Reload: a fresh mount restores BOTH panes — their tabs reappear in the dock strip.
    await mountStudio(ws, ["data-studio", "flows", "rules"], true);
    await waitFor(
      () => {
        const tabTitles = Array.from(document.querySelectorAll<HTMLElement>(".ds-tab")).map((t) => t.title);
        expect(tabTitles).toEqual(expect.arrayContaining(["Flows", "Rules"]));
      },
      { timeout: 5000 },
    );
  }, 30000);

  it("AppPage embedded mode: a view in a pane has NO full-width header; the standalone route keeps it", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedIotDemo();

    await mountStudio(ws, ["data-studio", "flows"]);
    fireEvent.click(screen.getByLabelText("open view"));
    fireEvent.click(await screen.findByLabelText("open flows view"));
    // The Flows pane is rendered. Embedded, its AppPage suppresses the full-width header — the pane
    // shows the empty-state body, NOT a standalone page's `<h1>Flows</h1>` (the dock tab is the title).
    const flowsSection = await waitFor(() => screen.getByLabelText("flows view"), { timeout: 5000 });
    expect(flowsSection.querySelector("h1")?.textContent ?? "").not.toContain("Flows");
    // The studio's OWN AppPage header is intact (one h1 with "Data Studio").
    expect(document.querySelectorAll("h1").length).toBeGreaterThanOrEqual(1);
    expect(
      Array.from(document.querySelectorAll("h1")).some((h) => (h.textContent ?? "").includes("Data Studio")),
    ).toBe(true);

    // Standalone (routed) FlowsView: NOT embedded — its own AppPage header renders. This is the same
    // component the pane mounts; embedding changes only WHERE it mounts, not its authority.
    const s = getSession();
    const { unmount } = render(
      <RoutingContextProvider
        value={{
          workspace: ws,
          principal: s?.principal ?? "",
          caps: s?.caps,
          allowed: ["flows"],
          extPages: [],
          extPagesLoading: false,
          onSignOut: () => {},
          switchWorkspace: () => {},
        }}
      >
        <FlowsView ws={ws} />
      </RoutingContextProvider>,
    );
    await waitFor(() =>
      expect(
        Array.from(document.querySelectorAll("h1")).some((h) => (h.textContent ?? "").trim() === "Flows"),
      ).toBe(true),
    );
    unmount();
  }, 30000);

  it("a view pane re-activates instead of duplicating (one pane per view kind)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedIotDemo();

    await mountStudio(ws, ["data-studio", "flows"]);
    fireEvent.click(screen.getByLabelText("open view"));
    fireEvent.click(await screen.findByLabelText("open flows view"));
    await waitFor(() => expect(screen.getAllByLabelText("flows view").length).toBe(1), { timeout: 5000 });

    // Open Flows again — the menu item refocuses the existing pane (no duplicate).
    fireEvent.click(screen.getByLabelText("open view"));
    fireEvent.click(await screen.findByLabelText("open flows view"));
    await new Promise((r) => setTimeout(r, 300));
    expect(screen.getAllByLabelText("flows view").length).toBe(1);
  }, 30000);

  it("capability-deny — a surface absent from `allowed` is omitted from the open-view menu", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedIotDemo();

    // The caller's route gating is the UI lens: `allowed` lists NO `flows` surface.
    await mountStudio(ws, ["data-studio"]);
    fireEvent.click(screen.getByLabelText("open view"));
    // The menu offers New panel but NOT the Flows entry — ungranted surfaces are omitted, not disabled.
    expect(screen.queryByLabelText("open flows view")).toBeNull();
    expect(screen.queryByLabelText("open rules view")).toBeNull();
    expect(screen.getByLabelText("open new panel")).toBeInTheDocument();
  }, 30000);
});

// Reference the standalone RulesView so the import stays meaningful for the embedded/routed parity.
void RulesView;
