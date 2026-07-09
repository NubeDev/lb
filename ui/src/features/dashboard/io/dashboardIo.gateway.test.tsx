// The dashboard import/export IO, driven against a REAL in-process gateway (dashboard scope; CLAUDE §9
// / testing §0 — no fake backend). Seeds a real dashboard + a real library panel through the shipped
// write verbs, serializes them into a portable bundle, then imports the bundle back through
// `useDashboardIo` and asserts the REAL store now holds the replayed records. Covers the mandatory
// workspace-isolation case (a bundle imported in ws-B lands in ws-B and is invisible to ws-A — the file
// carries no workspace) and the collision-safe rename policy. The pure parse/validate edge is unit-
// tested in `lib/dashboard/portable.test.ts`; this file exercises the store round-trip.

import { describe, expect, it, beforeAll } from "vitest";
import { renderHook, act } from "@testing-library/react";

import { useDashboardIo } from "./useDashboardIo";
import {
  saveDashboard,
  getDashboard,
  listDashboards,
  dashboardToPortable,
  makeBundle,
  serializeBundle,
  parseBundle,
} from "@/lib/dashboard";
import { listPanels, getPanel } from "@/lib/panel";
import { cellToSpec } from "@/lib/panel";
import type { Cell } from "@/lib/dashboard";
import { useRealGateway, signInReal } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `dash-io-${n++}`;

beforeAll(() => useRealGateway());

const cell: Cell = {
  i: "w1",
  x: 0,
  y: 0,
  w: 8,
  h: 4,
  v: 3,
  widget_type: "chart",
  view: "timeseries",
  binding: { series: "" },
  sources: [
    {
      refId: "A",
      tool: "series.read",
      args: { series: "cooler.temp" },
      datasource: { type: "surreal" },
    },
  ],
};

describe("dashboard IO (real gateway)", () => {
  it("round-trips: seed → export bundle → import → the store holds the replayed dashboard", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);

    // Seed a REAL dashboard through the shipped save verb (rule 9).
    await saveDashboard("ops", "Ops", [cell]);
    const full = await getDashboard("ops");

    // Build the exact bundle the export path produces.
    const bundle = makeBundle(
      [dashboardToPortable(full)],
      [],
      "2026-07-09T00:00:00Z",
    );
    const text = serializeBundle(bundle);

    // Re-parse it as an untrusted import would, then replay under the caller's authority. Rename mode
    // avoids clobbering the seed → the import lands as `ops-2`.
    const parsed = parseBundle(text);
    expect(parsed.ok).toBe(true);
    if (!parsed.ok) return;

    const { result } = renderHook(() => useDashboardIo());
    let outcome!: Awaited<ReturnType<typeof result.current.importBundle>>;
    await act(async () => {
      outcome = await result.current.importBundle(parsed.bundle, "rename");
    });

    expect(outcome.errors).toHaveLength(0);
    expect(outcome.dashboards).toHaveLength(1);
    expect(outcome.dashboards[0].renamedFrom).toBe("ops");
    const importedId = outcome.dashboards[0].id;
    expect(importedId).not.toBe("ops");

    // The REAL store now holds BOTH the seed and the import, with the same cells.
    const imported = await getDashboard(importedId);
    expect(imported.title).toBe("Ops");
    expect(imported.cells).toHaveLength(1);
    expect(imported.cells[0].sources?.[0]?.tool).toBe("series.read");

    const roster = await listDashboards();
    const ids = roster.map((d) => d.id.replace(/^dashboard:/, ""));
    expect(ids).toContain("ops");
    expect(ids).toContain(importedId);
  });

  it("carries a standalone widget (panel) and replays it through panel.save", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);

    const bundle = makeBundle(
      [],
      [{ id: "cooler-temp", title: "Cooler temp", spec: cellToSpec(cell) }],
      "2026-07-09T00:00:00Z",
    );
    const parsed = parseBundle(serializeBundle(bundle));
    expect(parsed.ok).toBe(true);
    if (!parsed.ok) return;

    const { result } = renderHook(() => useDashboardIo());
    await act(async () => {
      await result.current.importBundle(parsed.bundle, "rename");
    });

    const panels = await listPanels();
    const ids = panels.map((p) => p.id.replace(/^panel:/, ""));
    expect(ids).toContain("cooler-temp");
    const p = await getPanel("cooler-temp");
    expect(p.spec.view).toBe("timeseries");
  });

  it("WORKSPACE ISOLATION: a bundle imported in ws-B lands in ws-B and is invisible to ws-A", async () => {
    // The bundle carries NO workspace — authority comes from the token (rule 6). Author it while signed
    // into ws-A, then import while signed into ws-B; the record must exist ONLY in ws-B.
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await saveDashboard("secret", "Secret A", [cell]);
    const fullA = await getDashboard("secret");
    const text = serializeBundle(makeBundle([dashboardToPortable(fullA)], []));

    const wsB = nextWs();
    await signInReal("user:bob", wsB);
    const parsed = parseBundle(text);
    expect(parsed.ok).toBe(true);
    if (!parsed.ok) return;

    const { result } = renderHook(() => useDashboardIo());
    await act(async () => {
      // Overwrite mode keeps the id `secret` — proving it's a NEW ws-B record, not a cross-ws write.
      await result.current.importBundle(parsed.bundle, "overwrite");
    });

    // ws-B now has its own `secret`.
    const inB = await getDashboard("secret");
    expect(inB.title).toBe("Secret A");

    // Back in ws-A, the record is untouched and the roster never leaks ws-B's copy (there is only one
    // `secret` per workspace; the hard wall keeps them distinct records).
    await signInReal("user:ada", wsA);
    const rosterA = await listDashboards();
    const secrets = rosterA.filter(
      (d) => d.id.replace(/^dashboard:/, "") === "secret",
    );
    expect(secrets).toHaveLength(1);
  });
});
