// The OptionFocusPreview contract (panel-wizard scope, step 2): the wizard's per-option mini-preview
// renders the SAME `WidgetView` the dashboard renders (no second renderer = no drift), surfaced with an
// `optionFocus` marker that tags the region the focused option affects. This test pins that contract
// against a REAL gateway + REAL seeded rows — no fakes (rule 9).
//
// The headline: focus `decimals` + set `decimals: 2` → the preview's value readout shows "42.00". The
// underlying render is byte-identical to the dashboard's StatPanel; the wrapper only adds the marker.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";

import { useRealGateway, signInReal, seedSeries } from "@/test/gateway-session";
import type { Cell } from "@/lib/dashboard";
import { cellToEditorState, editorStateToCell } from "@/lib/panel-kit/cellEditorState";
import { writeOption } from "@/features/panel-builder/options/binding";
import { optionById } from "@/features/panel-builder/options/registry";
import { WithDashboardCache } from "@/features/dashboard/cache/testCacheWrapper";
import { OptionFocusPreview } from "@/features/panel-builder/options/OptionFocusPreview";

beforeAll(() => useRealGateway());

let n = 0;
const nextWs = () => `focusprev-${n++}`;

async function seedOne(series: string, payload: number): Promise<void> {
  await seedSeries({ series, seq: 1, payload, key: "kind", value: "temperature" });
}

/** Set `id` to `value` through the editor's REAL `writeOption` path — the same call the Field tab makes. */
function setOpt(cell: Cell, id: string, value: unknown): Cell {
  const def = optionById(id);
  if (!def) throw new Error(`unknown option ${id}`);
  const state = cellToEditorState(cell);
  const next = writeOption(state, def, value);
  return editorStateToCell({ ...state, ...next }, cell);
}

function baseStatCell(series: string): Cell {
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

describe("OptionFocusPreview — preview-per-option over the real WidgetView (real gateway)", () => {
  it("focus decimals + decimals:2 → the value readout renders 42.00 (no second renderer)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedOne("s.focus", 42);
    let cell = setOpt(baseStatCell("s.focus"), "decimals", 2);
    const { container, unmount } = render(
      <WithDashboardCache ws={ws}>
        <OptionFocusPreview cell={cell} workspace={ws} optionFocus={{ optionId: "decimals" }} />
      </WithDashboardCache>,
    );
    await waitFor(() => expect(screen.getByLabelText("stat value")).toBeInTheDocument());
    // The wrapper tagged the value region + the readout shows the formatted value (the SAME path StatPanel
    // uses; no second renderer).
    expect(container.querySelector(".option-focus-preview.focus-region-value")).not.toBeNull();
    expect(container.querySelector('[data-option-focus="decimals"]')).not.toBeNull();
    expect(screen.getByLabelText("stat value").textContent).toContain("42.00");
    unmount();
    cleanup();
  });

  it("no optionFocus → the wrapper renders the dashboard's plain WidgetView (the full-panel preview)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedOne("s.full", 42);
    const cell = setOpt(baseStatCell("s.full"), "decimals", 2);
    const { container, unmount } = render(
      <WithDashboardCache ws={ws}>
        <OptionFocusPreview cell={cell} workspace={ws} />
      </WithDashboardCache>,
    );
    await waitFor(() => expect(screen.getByLabelText("stat value")).toBeInTheDocument());
    // No region class — the wrapper is the plain full-panel preview.
    expect(container.querySelector(".focus-region-value")).toBeNull();
    expect(container.querySelector('[data-option-focus=""]')).not.toBeNull();
    // The value still renders through the real WidgetView.
    expect(screen.getByLabelText("stat value").textContent).toContain("42.00");
    unmount();
    cleanup();
  });

  it("a custom.* graph-style option tags the chart region (the canvas is the focus surface)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedOne("s.draw", 4);
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
      sources: [{ refId: "A", tool: "series.read", args: { series: "s.draw" }, datasource: { type: "series" } }],
      options: {},
    };
    const cell = setOpt(base, "custom.drawStyle", "bars");
    const { container, unmount } = render(
      <WithDashboardCache ws={ws}>
        <OptionFocusPreview cell={cell} workspace={ws} optionFocus={{ optionId: "custom.drawStyle" }} />
      </WithDashboardCache>,
    );
    await waitFor(() => expect(screen.getByLabelText("timeseries latest")).toBeInTheDocument());
    expect(container.querySelector(".option-focus-preview.focus-region-chart")).not.toBeNull();
    expect(container.querySelector('[data-option-focus="custom.drawStyle"]')).not.toBeNull();
    unmount();
    cleanup();
  });
});
