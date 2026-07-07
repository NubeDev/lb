// The viz gallery (data-studio-10x scope, phase 3 stage 2) — once rows exist, one thumbnail card per
// widget type replaces the text pill row. This unit test covers the type-mapping (6 chart-likes +
// 3 labeled cards = 9 type cards) and the shape-gating (a card the data can't honestly fill is
// disabled, not hidden — parity with `VizPicker`). The live-mini-render path (one `viz.query` for
// all thumbnails) is asserted at the gateway level in `DataStudioBuilderFlow.gateway.test.tsx`.

import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

import { VizGallery } from "./VizGallery";
import type { Cell, View } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import type { ResultShape } from "@/features/dashboard/views/shape";

// The gallery's thumbnail renders through `WidgetView`. For the unit, we stub it to a no-op so the
// test asserts the gallery's CARD structure (type-mapping + shape-gating) without driving a real
// query — the gateway suite proves the live-mini-render path.
vi.mock("@/features/dashboard/views/WidgetView", () => ({
  WidgetView: () => <div data-testid="widget-thumb" />,
}));

const cell: Cell = {
  i: "p",
  x: 0,
  y: 0,
  w: 8,
  h: 4,
  v: 3,
  widget_type: "chart",
  view: "timeseries",
  binding: { series: "" },
  sources: [{ refId: "A", tool: "series.read", args: { series: "x" }, datasource: { type: "series" } }],
};

const SCOPE: VarScope = { vars: {}, resolved: {} } as never;

function mountGallery(over: { view?: View; shape?: ResultShape } = {}) {
  const view: View = over.view ?? "timeseries";
  return render(
    <VizGallery
      cell={cell}
      ws="acme"
      scope={SCOPE}
      refreshKey={0}
      view={view}
      onChange={() => {}}
      shape={over.shape ?? "series"}
    />,
  );
}

describe("VizGallery — the type-mapping + shape-gating", () => {
  it("renders 9 type cards: 6 chart-likes (live mini-renders) + 3 labeled (table/genui/template)", () => {
    mountGallery();
    const cards = screen.getAllByRole("button", { name: /^viz / });
    expect(cards.length).toBe(9);
    // The chart-likes — the live-mini-render set.
    for (const v of ["timeseries", "barchart", "stat", "gauge", "bargauge", "piechart"]) {
      expect(screen.getByLabelText(`viz ${v}`)).toBeInTheDocument();
    }
    // The labeled cards — no mini-render (a Template thumbnail is noise).
    for (const v of ["table", "genui", "template"]) {
      expect(screen.getByLabelText(`viz ${v}`)).toBeInTheDocument();
    }
  });

  it("the selected view's card is aria-pressed", () => {
    mountGallery({ view: "gauge" });
    const gauge = screen.getByLabelText("viz gauge");
    expect(gauge).toHaveAttribute("aria-pressed", "true");
    // The non-selected cards are not pressed.
    expect(screen.getByLabelText("viz table")).toHaveAttribute("aria-pressed", "false");
  });

  it("shape-gating disables a card the data can't honestly fill (parity with VizPicker)", () => {
    // A `table` shape (multi-column tabular) cannot honestly fill a gauge — the card is disabled.
    mountGallery({ shape: "table" });
    const gauge = screen.getByLabelText("viz gauge") as HTMLButtonElement;
    expect(gauge.disabled).toBe(true);
    // The table card stays enabled (a tabular frame is always renderable as a grid).
    const table = screen.getByLabelText("viz table") as HTMLButtonElement;
    expect(table.disabled).toBe(false);
  });

  it("clicking an enabled card calls onChange with that view", () => {
    const onChange = vi.fn();
    render(
      <VizGallery
        cell={cell}
        ws="acme"
        scope={SCOPE}
        refreshKey={0}
        view="timeseries"
        onChange={onChange}
        shape="series"
      />,
    );
    fireEvent.click(screen.getByLabelText("viz stat"));
    expect(onChange).toHaveBeenCalledWith("stat");
  });

  it("clicking a shape-disabled card is a no-op (the disabled attribute enforces it)", () => {
    const onChange = vi.fn();
    render(
      <VizGallery
        cell={cell}
        ws="acme"
        scope={SCOPE}
        refreshKey={0}
        view="table"
        onChange={onChange}
        shape="table"
      />,
    );
    // Gauge is disabled for a table shape; clicking does nothing.
    fireEvent.click(screen.getByLabelText("viz gauge"));
    expect(onChange).not.toHaveBeenCalled();
  });

  it("the `unknown` shape (pre-data) leaves every card enabled — the picker stays permissive until data loads", () => {
    mountGallery({ shape: "unknown" });
    for (const v of ["timeseries", "barchart", "stat", "gauge", "bargauge", "piechart", "table"]) {
      expect((screen.getByLabelText(`viz ${v}`) as HTMLButtonElement).disabled).toBe(false);
    }
  });
});
