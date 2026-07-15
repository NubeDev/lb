// Registry dispatch: canonical-alias resolution, the ext:* wildcard, and the honest
// unknown-view placeholder (never a crash, never a fabricated widget).

import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import type { Cell } from "./dashboard.types";
import { createRegistry, EXT_WILDCARD } from "./registry";
import { DashboardStack } from "./Stack";

const cell = (i: string, view: string): Cell => ({
  i,
  x: 0,
  y: 0,
  w: 6,
  h: 4,
  v: 2,
  widget_type: "chart",
  view: view as Cell["view"],
  binding: { series: "s" },
});

describe("widget registry", () => {
  it("resolves through the canonical alias map (chart ⇒ timeseries renderer)", () => {
    const Ts = () => <div>ts</div>;
    const reg = createRegistry().register("timeseries", Ts);
    expect(reg.resolve("chart")).toBe(Ts);
    expect(reg.resolve("timeseries")).toBe(Ts);
    expect(reg.resolveCell(cell("a", "chart"))).toBe(Ts);
  });

  it("falls back to the ext:* wildcard for federation views", () => {
    const Ext = () => <div>ext</div>;
    const Exact = () => <div>exact</div>;
    const reg = createRegistry()
      .register(EXT_WILDCARD, Ext)
      .register("ext:mqtt/status", Exact);
    expect(reg.resolve("ext:mqtt/status")).toBe(Exact); // exact beats wildcard
    expect(reg.resolve("ext:github/prs")).toBe(Ext);
    expect(reg.resolve("stat")).toBeUndefined(); // the wildcard never catches non-ext views
  });

  it("dispatches a registered renderer with the cell", () => {
    const reg = createRegistry().register("stat", ({ cell: c }) => <div>stat:{c.i}</div>);
    render(<DashboardStack cells={[cell("s1", "stat")]} registry={reg} />);
    expect(screen.getByText("stat:s1")).toBeTruthy();
  });

  it("renders the honest placeholder for an unknown view — no crash, names the id", () => {
    const reg = createRegistry();
    render(<DashboardStack cells={[cell("x", "sparkline-9000")]} registry={reg} />);
    expect(screen.getByText(/No renderer for “sparkline-9000”/)).toBeTruthy();
  });
});
