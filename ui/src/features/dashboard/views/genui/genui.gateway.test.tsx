// GenUI over the REAL spawned gateway (genui-scope Testing plan, Integration). No fake backend — a real
// node, real `dashboard.save`/`dashboard.get`, real caps + workspace wall (CLAUDE §9). The model
// provider is the ONLY external and isn't touched here: these prove the DURABLE + RENDER paths, which
// carry no model — the author-side agent stream is proven separately (and jsdom has no EventSource).
//
// Covers:
//   - Host-side IR validation on save (Decision 6): a headless MCP author (this test IS one — it calls
//     the same `dashboard.save` verb) gets the LOUD rejection for a malformed genui cell.
//   - The empty-source v3 round-trip trap on a genui cell (the known gateway placeholder-`source` bug).
//   - Render WITHOUT the authoring adapter loaded: a saved genui cell reloads and renders through
//     `GenUiView` importing ONLY `@nube/genui` (render stratum) — never `@nube/genui/authoring`.
//   - Capability deny: no `dashboard.save` cap → the save is refused server-side.

import { describe, it, expect, beforeAll, vi } from "vitest";
import { render, waitFor } from "@testing-library/react";
import type { Cell } from "@/lib/dashboard";
import { getDashboard, saveDashboard } from "@/lib/dashboard/dashboard.api";
import { useRealGateway, signInReal, signInWithCaps } from "@/test/gateway-session";
// IMPORTANT: this file imports ONLY the render stratum — never `@nube/genui/authoring`. If GenUiView
// (transitively) needed the parser to render, this test would pull it in and the "render without the
// adapter" guarantee would be false. The WidgetView dispatch mounts GenUiView.
import { WidgetView } from "../WidgetView";

/** A well-formed genui cell: a `stat` bound to `/data/A/value`, fed by one v3 series target. */
function genuiCell(overrides: Partial<Cell> = {}): Cell {
  return {
    i: "g1",
    x: 0,
    y: 0,
    w: 6,
    h: 4,
    v: 3,
    widget_type: "chart",
    view: "genui",
    binding: { series: "" },
    sources: [{ refId: "A", tool: "series.watch", args: { series: "office/temp" } }],
    options: {
      genui: {
        v: 1,
        ir: {
          v: 1,
          surface: { surfaceId: "cell", root: "r" },
          components: {
            r: { id: "r", component: "stat", props: { value: { $bind: "/data/A/value" }, label: "Temp" } },
          },
        },
      },
    },
    ...overrides,
  };
}

let ws: string;
beforeAll(() => {
  useRealGateway();
});

describe("genui over the real gateway", () => {
  it("saves a well-formed genui cell, reloads it, and RENDERS without the adapter", async () => {
    ws = `ws-genui-${Date.now()}`;
    await signInReal("user:ada", ws);
    const saved = await saveDashboard("g", "Genui", [genuiCell()]);
    expect(saved.cells).toHaveLength(1);

    const reloaded = await getDashboard("g");
    const cell = reloaded.cells[0];
    expect(cell.view).toBe("genui");
    // Regression guard for the setState-in-render bug (debugging/genui/genui-probe-setstate-in-render):
    // the per-target data probe must report AFTER commit, not during render.
    const errSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    // Render the reloaded cell through the real dispatch — the label proves the IR rendered.
    const { container } = render(<WidgetView cell={cell} workspace={ws} />);
    await waitFor(() => expect(container.querySelector(".gu-root")).not.toBeNull());
    expect(container.textContent).toContain("Temp");
    expect(errSpy).not.toHaveBeenCalledWith(
      expect.stringContaining("Cannot update a component"),
      expect.anything(),
      expect.anything(),
    );
    errSpy.mockRestore();
  });

  it("SAVES an un-authored draft genui cell (adding an AI widget before generating it)", async () => {
    // The reported bug: picking "AI widget" adds a genui cell with no IR; saving it must NOT be
    // rejected (`options.genui is missing`) — it's a draft the author generates later.
    await signInReal("user:ada", `ws-genui-draft-${Date.now()}`);
    const draft = genuiCell({ sources: [], options: {} }); // no genui block yet
    await saveDashboard("gdraft", "Draft", [draft]); // must not throw
    const reloaded = await getDashboard("gdraft");
    const back = reloaded.cells[0];
    // It renders the author-me placeholder, not an error.
    const { container } = render(<WidgetView cell={back} workspace="x" />);
    await waitFor(() => expect(container.textContent).toMatch(/AI widget/i));
    expect(container.textContent).not.toMatch(/invalid/i);
  });

  it("REJECTS a malformed genui cell at save (Decision 6 — the headless-author loud rejection)", async () => {
    await signInReal("user:ada", `ws-genui-bad-${Date.now()}`);
    // An unknown component name — must be refused server-side, not degraded at view time.
    const bad = genuiCell();
    (bad.options as { genui: { ir: { components: Record<string, { component: string }> } } }).genui.ir.components.r.component =
      "Frobnicate";
    await expect(saveDashboard("gb", "Bad", [bad])).rejects.toThrow(/catalog|bad input/i);

    // An oversized spec is also refused.
    const big = genuiCell();
    (big.options as { genui: { ir: { components: Record<string, { props: Record<string, unknown> }> } } }).genui.ir.components.r.props.value =
      "x".repeat(9000);
    await expect(saveDashboard("gb2", "Big", [big])).rejects.toThrow(/too large|bad input/i);
  });

  it("round-trips a genui cell with the empty-source v3 trap intact (binding not broken)", async () => {
    await signInReal("user:ada", `ws-genui-empty-${Date.now()}`);
    // Simulate the gateway's known placeholder: a real v3 sources[] beside an empty v2 source.
    const cell = genuiCell({ source: { tool: "", args: undefined } });
    await saveDashboard("ge", "Empty", [cell]);
    const reloaded = await getDashboard("ge");
    const back = reloaded.cells[0];
    // The genui targets resolver must pick the real sources[], NOT the empty placeholder source.
    const { container } = render(<WidgetView cell={back} workspace="x" />);
    await waitFor(() => expect(container.querySelector(".gu-root")).not.toBeNull());
    // The stat still renders its label (binding resolved from sources[], not shadowed by empty source).
    expect(container.textContent).toContain("Temp");
  });

  it("DENIES a save without the dashboard.save cap (capability wall)", async () => {
    // A principal with read but NOT save.
    await signInWithCaps("user:bob", `ws-genui-deny-${Date.now()}`, ["mcp:dashboard.get:call"]);
    await expect(saveDashboard("gd", "Denied", [genuiCell()])).rejects.toThrow();
  });
});
