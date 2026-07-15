// The mobile stack: y,x reading order, row section dividers, collapsed members hidden,
// strictly read-only (no edit chrome ever).

import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import type { Cell } from "./dashboard.types";
import { createRegistry } from "./registry";
import { DashboardStack } from "./Stack";

const cell = (i: string, x: number, y: number, extra: Partial<Cell> = {}): Cell => ({
  i,
  x,
  y,
  w: 6,
  h: 4,
  v: 2,
  widget_type: "chart",
  view: "stat",
  binding: { series: "s" },
  ...extra,
});

const reg = createRegistry().register("stat", ({ cell: c }) => <div>w:{c.i}</div>);

describe("DashboardStack", () => {
  it("stacks cells in y,x order — the order the grid reads at full width", () => {
    // Deliberately shuffled input: right(6,0) before left(0,0), lower row first.
    const cells = [cell("low", 0, 8), cell("right", 6, 0), cell("left", 0, 0)];
    render(<DashboardStack cells={cells} registry={reg} />);
    const texts = screen
      .getAllByText(/^w:/)
      .map((n) => n.textContent);
    expect(texts).toEqual(["w:left", "w:right", "w:low"]);
  });

  it("renders a row as a plain section divider and hides a collapsed row's members", () => {
    const cells = [
      cell("r", 0, 0, { view: "row", w: 12, h: 1, title: "Section A", options: { collapsed: true } }),
      cell("hidden", 0, 1),
      cell("r2", 0, 6, { view: "row", w: 12, h: 1, title: "Section B" }),
      cell("shown", 0, 7),
    ];
    render(<DashboardStack cells={cells} registry={reg} />);
    expect(screen.getByText("Section A")).toBeTruthy();
    expect(screen.getByText("Section B")).toBeTruthy();
    expect(screen.queryByText("w:hidden")).toBeNull();
    expect(screen.getByText("w:shown")).toBeTruthy();
  });

  it("is read-only: no drag handles, no remove/duplicate chrome", () => {
    render(<DashboardStack cells={[cell("a", 0, 0)]} registry={reg} />);
    expect(screen.queryByLabelText("move cell a")).toBeNull();
    expect(screen.queryByLabelText("remove cell a")).toBeNull();
  });
});
