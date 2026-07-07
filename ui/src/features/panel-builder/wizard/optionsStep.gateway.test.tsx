// OptionsStep + useWizardPreview — the one-preview contract + the cost model (panel-wizard scope,
// redesigned per resolved decision #3). The Options step is a compact form with NO chart of its own;
// the host mounts ONE `OptionFocusPreview` beside it and points its `optionFocus` at the option being
// edited. This suite pins, against a REAL gateway + REAL seeded rows (rule 9):
//   - exactly ONE preview mounts, no matter how many options are registered;
//   - a LIVE option toggle changes that one preview; editing a row focuses the preview on that option;
//   - a DEAD option still surfaces its honest "renderer pending" note in the list;
//   - the cost model: a presentation-option toggle re-keys ONLY the SHAPE pass (frames-in), never the
//     FETCH — counted through an `ipc.invoke` spy that DELEGATES to the real transport (observe, never fake).

import { describe, expect, it, beforeAll, vi } from "vitest";
import { render, screen, waitFor, cleanup, fireEvent } from "@testing-library/react";
import * as ipc from "@/lib/ipc/invoke";

import { useRealGateway, signInReal, seedSeries } from "@/test/gateway-session";
import type { Cell } from "@/lib/dashboard";
import { cellToEditorState } from "@/lib/panel-kit/cellEditorState";
import { WithDashboardCache } from "@/features/dashboard/cache/testCacheWrapper";
import { OptionsStep } from "@/features/panel-builder/wizard/OptionsStep";
import { OptionFocusPreview } from "@/features/panel-builder/options/OptionFocusPreview";
import { useWizardPreview } from "@/features/panel-builder/wizard/useWizardPreview";
import type { EditorState } from "@/lib/panel-kit/cellEditorState";
import { useState } from "react";

beforeAll(() => useRealGateway());

let n = 0;
const nextWs = () => `optstep-${n++}`;

async function seedOne(series: string, payload: number): Promise<void> {
  await seedSeries({ series, seq: 1, payload, key: "kind", value: "temperature" });
}

/** Count `viz.query` mcp_call invocations around a render — DELEGATING to the real transport (rule 9).
 *  Distinguishes FETCH (args.panel.sources — a datasource round-trip) from SHAPE (args.panel.frames — a
 *  compute-only frames-in pass; the cost-model's "no re-fetch" half). */
function withVizCounter<T>(runFn: () => T): {
  result: T;
  fetches: () => number;
  shapes: () => number;
  restore: () => void;
} {
  const real = ipc.invoke;
  let fetches = 0;
  let shapes = 0;
  const spy = vi.spyOn(ipc, "invoke").mockImplementation(((cmd: string, args?: Record<string, unknown>) => {
    if (cmd === "mcp_call" && (args?.tool as string) === "viz.query") {
      const panel = (args?.args as { panel?: Record<string, unknown> } | undefined)?.panel;
      if (panel && Array.isArray(panel.frames)) shapes++;
      else fetches++;
    }
    return real(cmd, args);
  }) as typeof ipc.invoke);
  const result = runFn();
  return { result, fetches: () => fetches, shapes: () => shapes, restore: () => spy.mockRestore() };
}

/** A host shell mirroring PanelWizard's options-step binding: the compact form on the left + the ONE
 *  pinned OptionFocusPreview, pointed at the focused option. */
function OptionsStepHarness({ ws, initial }: { ws: string; initial: Cell }) {
  const [state, setState] = useState<EditorState>(() => cellToEditorState(initial));
  const [focused, setFocused] = useState<string | undefined>(undefined);
  const preview = useWizardPreview(state);
  const patch = (next: Partial<EditorState>) => setState((s) => ({ ...s, ...next }));
  return (
    <>
      <OptionsStep state={state} patch={patch} onFocusOption={setFocused} focusedOption={focused} />
      <OptionFocusPreview
        cell={preview.cell}
        workspace={ws}
        refreshKey={preview.refreshKey}
        optionFocus={focused ? { optionId: focused } : undefined}
      />
    </>
  );
}

function statCell(series: string): Cell {
  return {
    i: "c",
    x: 0,
    y: 0,
    w: 6,
    h: 4,
    v: 3,
    widget_type: "stat",
    view: "stat",
    binding: { series: "" },
    sources: [{ refId: "A", tool: "series.read", args: { series }, datasource: { type: "series" } }],
    options: { reduceOptions: { calcs: ["lastNotNull"] }, colorMode: "value", graphMode: "none", textMode: "auto" },
  };
}

describe("OptionsStep — one pinned preview + the cost model (real gateway)", () => {
  it("exactly ONE preview mounts; editing a LIVE option (decimals) changes it + focuses it on the option", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedOne("cooler.temp", 42);
    const { container, unmount } = render(
      <WithDashboardCache ws={ws}>
        <OptionsStepHarness ws={ws} initial={statCell("cooler.temp")} />
      </WithDashboardCache>,
    );
    // ONE preview, ONE value readout — the option rows mount no chart of their own.
    await waitFor(() => expect(screen.getAllByLabelText("stat value").length).toBe(1));
    expect(container.querySelectorAll(".option-focus-preview").length).toBe(1);
    expect(screen.getByLabelText("stat value").textContent).toContain("42");

    // Type decimals=2 — the row reports focus (the preview points at "decimals") and the ONE preview
    // re-renders the formatted value.
    const decimalsInput = screen.getByLabelText("Decimals") as HTMLInputElement;
    fireEvent.focus(decimalsInput);
    fireEvent.change(decimalsInput, { target: { value: "2" } });
    await waitFor(() => expect(screen.getByLabelText("stat value").textContent).toContain("42.00"));
    expect(container.querySelector(".option-focus-preview")?.getAttribute("data-option-focus")).toBe("decimals");
    // The focused row is highlighted in the list.
    expect(container.querySelector('[data-option-id="decimals"]')?.className).toContain("bg-accent");
    unmount();
    cleanup();
  });

  it("DEAD option (custom.spanNulls on timeseries): the row carries the honest 'renderer pending' note", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedOne("cooler.temp", 4);
    const base: Cell = {
      i: "c",
      x: 0,
      y: 0,
      w: 6,
      h: 4,
      v: 3,
      widget_type: "chart",
      view: "timeseries",
      binding: { series: "" },
      sources: [{ refId: "A", tool: "series.read", args: { series: "cooler.temp" }, datasource: { type: "series" } }],
      options: {},
    };
    const { container, unmount } = render(
      <WithDashboardCache ws={ws}>
        <OptionsStepHarness ws={ws} initial={base} />
      </WithDashboardCache>,
    );
    await waitFor(() => expect(screen.getAllByLabelText("timeseries latest").length).toBe(1));
    // Non-first groups start collapsed (the anti-overwhelm disclosure) — expand Graph styles first.
    fireEvent.click(screen.getByLabelText("toggle group Graph styles"));
    const row = container.querySelector('[data-option-id="custom.spanNulls"]');
    expect(row?.getAttribute("data-live")).toBe("false");
    expect(row?.textContent).toMatch(/no visible effect/i);
    // Still only the ONE preview — a timeseries registers ~20 options and none of them mount a chart.
    expect(container.querySelectorAll(".option-focus-preview").length).toBe(1);
    unmount();
    cleanup();
  });

  it("the cost model: a presentation-option toggle does NOT re-fetch; the panel's first render fetches once", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedOne("cooler.temp", 42);
    const counter = withVizCounter(() =>
      render(
        <WithDashboardCache ws={ws}>
          <OptionsStepHarness ws={ws} initial={statCell("cooler.temp")} />
        </WithDashboardCache>,
      ),
    );
    // The initial mount fetches once (the ONE preview's first read).
    await waitFor(() => expect(screen.getAllByLabelText("stat value").length).toBe(1));
    const fetchesAfterMount = counter.fetches();
    expect(fetchesAfterMount).toBeGreaterThanOrEqual(1);

    // Toggle a presentation option (decimals) — this must NOT trigger another FETCH (the shape pass may
    // fire as a frames-in reshape, but it is NOT a datasource round-trip). The cost-model guarantee.
    const decimalsInput = screen.getByLabelText("Decimals") as HTMLInputElement;
    fireEvent.change(decimalsInput, { target: { value: "2" } });
    await waitFor(() => expect(screen.getByLabelText("stat value").textContent).toContain("42.00"));
    expect(counter.fetches(), "a presentation toggle must not fire a datasource fetch").toBe(fetchesAfterMount);

    counter.restore();
    counter.result.unmount();
    cleanup();
  });
});
