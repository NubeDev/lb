// Library panels, driven against a REAL spawned gateway (CLAUDE §9 — no fake). Proves the scope's UI
// plan end to end over real `panel.*`/`dashboard.*` routes, each re-checked server-side:
//  - CRUD round-trip through the real `panel.*` client;
//  - capability-deny (a caller without `panel.save` cannot persist) — mandatory;
//  - workspace-isolation (ws-B cannot read a ws-A panel) — mandatory;
//  - save-as-library → reuse on a dashboard (a ref cell) → edit-once-PROPAGATES on the next
//    `dashboard.get` (host-side hydration) + the echoed-spec-is-ignored (ref authoritative);
//  - the cross-ws `panel_ref` is REJECTED at save (validate-at-write) — the isolation headline;
//  - a dangling ref hydrates to the placeholder (`panelMissing`);
//  - the standalone `/panel/{id}` page renders ONE panel full-bleed through the shipped render path.
// Each test logs into a UNIQUE workspace for isolation on the shared node.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

import { PanelPage } from "./PanelPage";
import { CAP } from "@/lib/session/admin-caps";
import { getPanel, listPanels, savePanel, deletePanel, sharePanel, panelUsage } from "@/lib/panel";
import { saveDashboard, getDashboard } from "@/lib/dashboard";
import type { PanelSpec } from "@/lib/panel";
import type { Cell } from "@/lib/dashboard";
import { useRealGateway, signInReal, signInWithCaps } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `panel-${n++}`;

/** A minimal v3 spec (a stat over a static option) — enough to round-trip + render. */
function spec(view = "timeseries"): PanelSpec {
  return {
    v: 3,
    widget_type: "chart",
    title: "Cooler temp",
    view,
    binding: { series: "" },
    options: {},
  } as PanelSpec;
}

/** A ref cell carrying a deliberately WRONG echoed spec — must be ignored (ref authoritative). */
function refCell(i: string, panelId: string): Cell {
  return {
    i,
    x: 0,
    y: 0,
    w: 8,
    h: 4,
    widget_type: "chart",
    binding: { series: "" },
    view: "STALE",
    panelRef: `panel:${panelId}`,
  };
}

beforeAll(() => useRealGateway());

describe("library panels (real gateway)", () => {
  it("CRUD round-trips through the real panel.* routes", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);

    const saved = await savePanel("cooler", "Cooler", spec());
    expect(saved.owner).toBe("user:ada");
    expect(saved.spec.view).toBe("timeseries");

    const roster = await listPanels();
    expect(roster.find((p) => p.id === "cooler")?.view).toBe("timeseries");

    const got = await getPanel("cooler");
    expect(got.title).toBe("Cooler");

    await deletePanel("cooler");
    await expect(getPanel("cooler")).rejects.toBeTruthy();
  });

  it("denies panel.save without the cap (nothing persists) — mandatory", async () => {
    const ws = nextWs();
    // Only the read caps, NOT panel.save.
    await signInWithCaps("user:ben", ws, [CAP.panelGet, CAP.panelList]);
    await expect(savePanel("x", "X", spec())).rejects.toBeTruthy();
    // And nothing was written.
    await signInWithCaps("user:ben", ws, [CAP.panelList]);
    expect((await listPanels()).find((p) => p.id === "x")).toBeUndefined();
  });

  it("workspace-isolates panels — ws-B cannot read a ws-A panel (mandatory)", async () => {
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await savePanel("secret", "Secret", spec());

    const wsB = nextWs();
    await signInReal("user:ada", wsB); // same user, DIFFERENT workspace
    await expect(getPanel("secret")).rejects.toBeTruthy();
    expect((await listPanels()).find((p) => p.id === "secret")).toBeUndefined();
  });

  it("save-as-library → reuse on a dashboard → edit-once-PROPAGATES (host hydration, echoed spec ignored)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);

    await savePanel("cooler", "Cooler", spec("timeseries"));
    // A dashboard with a ref cell carrying a STALE echoed spec.
    await saveDashboard("ops", "Ops", [refCell("c1", "cooler")]);

    // dashboard.get hydrates the ref from the panel record — NOT the "STALE" echoed spec.
    let d = await getDashboard("ops");
    expect(d.cells[0].panelRef).toBe("panel:cooler");
    expect(d.cells[0].view).toBe("timeseries");
    expect(d.cells[0].view).not.toBe("STALE");

    // usage reports the referencing dashboard (the editor banner source).
    expect((await panelUsage("cooler")).map((u) => u.dashboard)).toEqual(["ops"]);

    // Edit the panel ONCE → the dashboard reflects it on next load (edit-once-reuse).
    await savePanel("cooler", "Cooler", spec("gauge"));
    d = await getDashboard("ops");
    expect(d.cells[0].view).toBe("gauge");
  });

  it("rejects a cross-workspace panel_ref at save (validate-at-write) — the isolation headline", async () => {
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await savePanel("cooler", "Cooler", spec());

    const wsB = nextWs();
    await signInReal("user:ada", wsB);
    // The panel lives in ws-A only; a ws-B dashboard referencing it must be REJECTED loudly.
    await expect(saveDashboard("b", "B", [refCell("c1", "cooler")])).rejects.toBeTruthy();
  });

  it("hydrates a dangling ref to the placeholder (panelMissing)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await savePanel("cooler", "Cooler", spec());
    await saveDashboard("ops", "Ops", [refCell("c1", "cooler")]);
    await deletePanel("cooler", true); // force-delete while referenced

    const d = await getDashboard("ops");
    expect(d.cells[0].panelMissing).toBe(true);
    expect(d.cells[0].panelRef).toBe("panel:cooler");
    // No spec leaked on a missing panel.
    expect(d.cells[0].sources ?? []).toEqual([]);
  });

  it("shares the DEFINITION only — a viewer reads the shared panel record (a lens, not a grant)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await savePanel("cooler", "Cooler", spec());
    await sharePanel("cooler", "workspace");

    // Ben (workspace member) holds only the panel read cap — he reads the DEFINITION. The data its
    // sources[] read is independently re-checked under his caps at render (backend-proven); sharing the
    // panel widened no data access.
    await signInWithCaps("user:ben", ws, [CAP.panelGet]);
    const got = await getPanel("cooler");
    expect(got.title).toBe("Cooler");
  });

  it("renders the standalone /panel/{id} page full-bleed through the shipped render path", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await savePanel("cooler", "Cooler temp", spec());

    render(<PanelPage ws={ws} id="cooler" range={{ from: "2026-07-01", to: "2026-07-03" }} />);
    // The page fetches the panel and renders it full-bleed (the AppPage title + the render host).
    expect(await screen.findByText("Cooler temp")).toBeInTheDocument();
    await waitFor(() => expect(screen.getByTestId("standalone-panel")).toBeInTheDocument());
  });
});
