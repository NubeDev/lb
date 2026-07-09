// Save (panel-wizard scope, step 8) — the wizard's trailing action. Serializes the wizard's `EditorState`
// through `editorStateToCell` (the SAME path the editor's Save uses — the no-drift guarantee is
// structural), appends to the dashboard's cells, persists via `dashboard.save`. The host re-checks
// `mcp:dashboard.save:call` on save (the wizard's only cap; no new verb, no new table).
//
// This test suite covers the testing plan's mandatory categories that bear on Save:
//   - the **no-drift invariant** (headline): building a panel through the WIZARD's writeOption path
//     AND through the EDITOR's path for the same options produces the SAME serialized cell;
//   - the **save round-trip**: a wizard-built panel survives a `dashboard.save` → `dashboard.get`;
//   - the **edit-cap gate + host backstop**: a session without `mcp:dashboard.save:call` cannot save
//     (the host denies server-side, opaque);
//   - **workspace isolation**: a ws-B wizard's save target never crosses into ws-A.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { useRealGateway, signInReal, signInWithCaps, seedSeries } from "@/test/gateway-session";
import { getDashboard, saveDashboard } from "@/lib/dashboard/dashboard.api";
import type { Cell } from "@/lib/dashboard";
import { cellToEditorState, editorStateToCell } from "@/lib/panel-kit/cellEditorState";
import { defaultCell } from "@/lib/panel-kit/defaultCell";
import { writeOption } from "@/features/panel-builder/options/binding";
import { optionById } from "@/features/panel-builder/options/registry";
import { WithDashboardCache } from "@/features/dashboard/cache/testCacheWrapper";
import { PanelWizard } from "@/features/panel-builder/wizard/PanelWizard";

beforeAll(() => useRealGateway());

let n = 0;
const nextWs = () => `pwizsave-${n++}`;

async function seedOne(series: string, payload: number): Promise<void> {
  await seedSeries({ series, seq: 1, payload, key: "kind", value: "temperature" });
}

/** Build a panel by writing options through the registry's `writeOption` (the Field-tab + wizard path). */
function buildViaWriteOption(view: Cell["view"], series: string, opts: Array<[string, unknown]>, base?: Cell): Cell {
  let cell = base ?? {
    i: "c",
    x: 0,
    y: 0,
    w: 6,
    h: 4,
    v: 3,
    widget_type: view === "timeseries" ? "chart" : "stat",
    view,
    binding: { series: "" },
    sources: [{ refId: "A", tool: "series.read", args: { series }, datasource: { type: "series" } }],
    options: view === "stat" ? { reduceOptions: { calcs: ["lastNotNull"] }, colorMode: "value", graphMode: "none", textMode: "auto" } : {},
  };
  for (const [id, value] of opts) cell = setOpt(cell, id, value);
  return cell;
}

function setOpt(cell: Cell, id: string, value: unknown): Cell {
  const def = optionById(id);
  if (!def) throw new Error(`unknown option ${id}`);
  const state = cellToEditorState(cell);
  const next = writeOption(state, def, value);
  return editorStateToCell({ ...state, ...next }, cell);
}

describe("PanelWizard Save — the no-drift invariant + cap gate + isolation (real gateway)", () => {
  it("NO-DRIFT HEADLINE: a panel built through writeOption serializes identically from a cellToEditorState round-trip", () => {
    // The no-drift proof: editorStateToCell(cellToEditorState(c)) ≡ c for any options set written via the
    // registry's writeOption — the same path the wizard and the Field tab share. The wizard is a thin
    // shell; whatever it writes, the editor writes byte-identically.
    const built = buildViaWriteOption("stat", "s.x", [["decimals", 2], ["unit", "celsius"]]);
    const roundTrip = editorStateToCell(cellToEditorState(built), built);
    // Strip the cell key (the wizard assigns its own at save; not a drift signal).
    expect({ ...roundTrip, i: built.i }).toEqual(built);
  });

  it("SAVE ROUND-TRIP: a wizard-built panel survives dashboard.save → dashboard.get (the persisted cell renders)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedOne("cooler.temp", 42);
    // Author a dashboard to save into.
    const dash = await saveDashboard("d-wiz", "Ops", []);
    expect(dash.cells.length).toBe(0);

    // Build the panel through the wizard's writeOption path + serialize through editorStateToCell.
    const built = buildViaWriteOption("stat", "cooler.temp", [["decimals", 2]]);
    const draftBase = defaultCell("stat", "wizard-draft");
    const cell = editorStateToCell(cellToEditorState(built), draftBase);
    const placed: Cell = { ...cell, i: "panel-1", x: 0, y: 0 };

    // Save + reload — the persisted cell round-trips through the host.
    await saveDashboard("d-wiz", "Ops", [placed]);
    const reloaded = await getDashboard("d-wiz");
    expect(reloaded.cells.length).toBe(1);
    expect(reloaded.cells[0]!.view).toBe("stat");
    expect(reloaded.cells[0]!.fieldConfig?.defaults?.decimals).toBe(2);
  });

  it("CAP GATE + HOST BACKSTOP: a session without dashboard.save cannot save (the host denies, opaque)", async () => {
    const ws = nextWs();
    // Sign in WITHOUT the dashboard.save cap.
    await signInWithCaps("user:viewer", ws, []);
    // The host refuses `dashboard_save` for this identity — the wizard's UI gate (DefaultRedirect at the
    // route) keeps the surface unreachable, and the gateway is the wall regardless.
    await expect(saveDashboard("d-deny", "X", [])).rejects.toThrow();
  });

  it("WORKSPACE ISOLATION: a ws-B wizard's save target never crosses into ws-A", async () => {
    const wsA = nextWs();
    const wsB = nextWs();
    await signInReal("user:ada", wsA);
    await saveDashboard("d-shared-id", "A-board", []);
    // ws-B ada signs in + saves to the SAME dashboard id — the host re-derives ws-B's workspace from the
    // token, so ws-B's save targets ws-B's `d-shared-id`, never ws-A's.
    await signInReal("user:ada", wsB);
    const built = buildViaWriteOption("stat", "s.b", [["decimals", 1]]);
    const cell = editorStateToCell(cellToEditorState(built), defaultCell("stat", "w"));
    await saveDashboard("d-shared-id", "B-board", [{ ...cell, i: "p", x: 0, y: 0 }]);
    // ws-A's `d-shared-id` is untouched.
    await signInReal("user:ada", wsA);
    const aDash = await getDashboard("d-shared-id");
    expect(aDash.title).toBe("A-board");
    expect(aDash.cells.length).toBe(0);
  });

  it("the wizard's Save button persists the panel into the dashboard it was opened for", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedOne("cooler.temp", 42);
    await saveDashboard("d-button", "Ops", []);
    const user = userEvent.setup();
    render(
      <WithDashboardCache ws={ws}>
        <PanelWizard ws={ws} dashboardId="d-button" onExit={() => {}} />
      </WithDashboardCache>,
    );
    // Pick a source.
    await user.click(await screen.findByLabelText("source track workspace"));
    await user.click(screen.getByLabelText("wizard source"));
    const opt = await screen.findByRole("option", { name: "cooler.temp" });
    await user.click(opt);
    await waitFor(() => expect(screen.getByLabelText("timeseries latest")).toBeInTheDocument());
    // Walk to the transform step (where Save lives): Source → ChartType → Options → Transform.
    await user.click(screen.getByText("Next"));
    await waitFor(() => expect(screen.getByLabelText("wizard chart-type step")).toBeInTheDocument());
    await user.click(screen.getByText("Next"));
    await waitFor(() => expect(screen.getByLabelText("wizard options step")).toBeInTheDocument());
    await user.click(screen.getByText("Next"));
    await waitFor(() => expect(screen.getByLabelText("wizard transform step")).toBeInTheDocument());
    // Save.
    await user.click(screen.getByLabelText("save panel"));
    // The dashboard now has the panel.
    await waitFor(async () => {
      const d = await getDashboard("d-button");
      expect(d.cells.length).toBe(1);
    });
    expect((await getDashboard("d-button")).cells[0]!.view).toBe("timeseries");
    cleanup();
  });

  it("EDIT MODE: opening the wizard with editCell replaces the cell in place (same key + geometry), not append", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedOne("cooler.temp", 42);
    // An existing panel on the dashboard at a fixed key + geometry (what the dashboard grid holds).
    const existing = buildViaWriteOption("stat", "cooler.temp", [["decimals", 0]]);
    const placed: Cell = { ...editorStateToCell(cellToEditorState(existing), defaultCell("stat", "w")), i: "w7", x: 3, y: 5, w: 6, h: 4 };
    await saveDashboard("d-edit", "Ops", [placed]);

    const user = userEvent.setup();
    render(
      <WithDashboardCache ws={ws}>
        <PanelWizard ws={ws} dashboardId="d-edit" editCell={placed} onExit={() => {}} />
      </WithDashboardCache>,
    );
    // Edit mode seeds from the existing cell — the header says "Edit panel".
    expect(screen.getByText("Edit panel")).toBeInTheDocument();
    // Walk to Save (no re-pick needed; the source is already bound) and persist.
    await waitFor(() => expect(screen.getByLabelText("wizard source step")).toBeInTheDocument());
    await user.click(screen.getByText("Next"));
    await waitFor(() => expect(screen.getByLabelText("wizard chart-type step")).toBeInTheDocument());
    await user.click(screen.getByText("Next"));
    await waitFor(() => expect(screen.getByLabelText("wizard options step")).toBeInTheDocument());
    await user.click(screen.getByText("Next"));
    await waitFor(() => expect(screen.getByLabelText("wizard transform step")).toBeInTheDocument());
    await user.click(screen.getByLabelText("save panel"));

    // Replaced IN PLACE: still ONE cell, same key + geometry — not appended as a second panel.
    await waitFor(async () => {
      const d = await getDashboard("d-edit");
      expect(d.cells.length).toBe(1);
    });
    const cell = (await getDashboard("d-edit")).cells[0]!;
    expect(cell.i).toBe("w7");
    expect([cell.x, cell.y, cell.w, cell.h]).toEqual([3, 5, 6, 4]);
    cleanup();
  });
});
