// Panel rows against a REAL in-process gateway (panel-rows scope, "Gateway" + the mandatory
// deny/isolation/additivity tests; CLAUDE §9 / testing §0 — no fake backend). A row is a
// `Cell{ view:"row" }` riding the shipped `dashboard.save`/`dashboard.get` — NO new verb, NO new cap.
// We seed real row + member cells through the real write path and assert: the row cell + its
// `options.collapsed` byte round-trips; toggling collapse persists; a pre-rows dashboard loads
// unchanged (additivity); saving a row cell without `dashboard.save` is denied server-side; a ws-A
// dashboard with rows is invisible to ws-B.

import { describe, expect, it, beforeAll } from "vitest";

import { saveDashboard, getDashboard, listDashboards } from "@/lib/dashboard";
import type { Cell } from "@/lib/dashboard";
import { rowMembers, isCollapsed, ROW_W, ROW_H } from "@/lib/dashboard";
import { useRealGateway, signInReal, signInWithCaps } from "@/test/gateway-session";
import { CAP } from "@/lib/session";

let n = 0;
const nextWs = () => `dash-rows-${n++}`;

beforeAll(() => useRealGateway());

/** A row header cell (panel-rows). Full-width, short, `view:"row"`; `collapsed` rides `options`. */
function rowCell(i: string, y: number, title: string, collapsed = false): Cell {
  return {
    i,
    x: 0,
    y,
    w: ROW_W,
    h: ROW_H,
    widget_type: "chart",
    view: "row",
    binding: { series: "" },
    title,
    ...(collapsed ? { options: { collapsed: true } } : {}),
  };
}

/** An ordinary member panel below a row. */
function panelCell(i: string, y: number): Cell {
  return {
    i,
    x: 0,
    y,
    w: 6,
    h: 4,
    v: 3,
    widget_type: "chart",
    view: "timeseries",
    binding: { series: "" },
    sources: [
      { refId: "A", tool: "series.read", args: { series: "cooler.temp" }, datasource: { type: "surreal" } },
    ],
  };
}

describe("panel rows — gateway round-trip", () => {
  it("a row cell + its members + collapsed flag round-trip byte-clean through dashboard.save/get", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);

    const cells = [rowCell("r1", 0, "Section A"), panelCell("a", 1), panelCell("b", 2)];
    await saveDashboard("ops", "Ops", cells);

    const got = await getDashboard("ops");
    const row = got.cells.find((c) => c.i === "r1");
    expect(row?.view).toBe("row");
    expect(row?.title).toBe("Section A");
    // Positional membership survives the round-trip (no rowId — geometry is the source of truth).
    expect(rowMembers(got.cells, row!).map((c) => c.i)).toEqual(["a", "b"]);
    expect(isCollapsed(row!)).toBe(false);
  });

  it("toggling collapse persists (options.collapsed round-trips true)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);

    await saveDashboard("ops", "Ops", [rowCell("r1", 0, "S"), panelCell("a", 1)]);
    // Author flips collapse (what the header's chevron does via dash.saveCells).
    await saveDashboard("ops", "Ops", [rowCell("r1", 0, "S", true), panelCell("a", 1)]);

    const got = await getDashboard("ops");
    expect(isCollapsed(got.cells.find((c) => c.i === "r1")!)).toBe(true);
    // The member keeps its real geometry (collapse is a render flag, not a geometry rewrite).
    expect(got.cells.find((c) => c.i === "a")?.y).toBe(1);
  });

  it("delete row-only leaves the members (positional — they merge into the region)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);

    const cells = [rowCell("r1", 0, "S"), panelCell("a", 1), panelCell("b", 2)];
    await saveDashboard("ops", "Ops", cells);
    // Row-only delete = drop the header cell (DashboardView's onRemove default).
    await saveDashboard("ops", "Ops", cells.filter((c) => c.i !== "r1"));

    const got = await getDashboard("ops");
    expect(got.cells.map((c) => c.i).sort()).toEqual(["a", "b"]);
  });
});

describe("panel rows — additivity / deny / isolation (mandatory)", () => {
  it("ADDITIVITY: a pre-rows dashboard (no view:row cell) loads and round-trips unchanged", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);

    const cells = [panelCell("a", 0), panelCell("b", 4)];
    await saveDashboard("legacy", "Legacy", cells);

    const got = await getDashboard("legacy");
    expect(got.cells.map((c) => c.i)).toEqual(["a", "b"]);
    // No row present → rowMembers of nothing; the board is entirely ungrouped, exactly as before.
    expect(got.cells.some((c) => c.view === "row")).toBe(false);
  });

  it("DENY (mandatory, server-side): saving a dashboard with a row cell without dashboard.save is refused", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveDashboard("ops", "Ops", [panelCell("a", 0)]);

    // A token with the reads only — no save. The row cell is ordinary bytes; the deny is the SAME
    // opaque cap wall as any dashboard save (no new cap for rows).
    await signInWithCaps("user:ben", ws, [CAP.dashboardList, CAP.dashboardGet]);
    await expect(
      saveDashboard("ops", "Hijacked", [rowCell("r1", 0, "Sneaky"), panelCell("a", 1)]),
    ).rejects.toThrow();
  });

  it("ISOLATION (mandatory): a ws-A dashboard with rows is invisible to ws-B", async () => {
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await saveDashboard("ops", "Ops", [rowCell("r1", 0, "Secret"), panelCell("a", 1)]);
    expect((await listDashboards()).some((d) => d.id === "ops")).toBe(true);

    const wsB = nextWs();
    await signInReal("user:ben", wsB);
    expect((await listDashboards()).some((d) => d.id === "ops")).toBe(false);
    await expect(getDashboard("ops")).rejects.toThrow();
  });
});
