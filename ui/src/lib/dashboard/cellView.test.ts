// cellView — resolve a cell's effective render view. The load-bearing default (data-studio-ux): a cell
// with NEITHER a `view` nor a `widget_type` (a malformed / half-authored cell — e.g. a federation panel
// left with none) must default to `timeseries`, not render "unsupported view:". A real but unknown view
// still falls through unchanged (an honest unsupported state — the default doesn't mask it).

import { describe, it, expect } from "vitest";
import { cellView } from "./dashboard.types";
import type { Cell } from "./dashboard.types";

const base = (over: Partial<Cell>): Cell =>
  ({ i: "c", x: 0, y: 0, w: 6, h: 4, widget_type: "chart", binding: { series: "" }, ...over }) as Cell;

describe("cellView", () => {
  it("uses the explicit v2 view, canonicalized (chart → timeseries)", () => {
    expect(cellView(base({ view: "chart" }))).toBe("timeseries");
    expect(cellView(base({ view: "stat" }))).toBe("stat");
  });

  it("falls back to widget_type when no view is set", () => {
    expect(cellView(base({ view: undefined, widget_type: "chart" }))).toBe("timeseries");
  });

  it("defaults to timeseries when BOTH view and widget_type are empty (no 'unsupported view:')", () => {
    expect(cellView(base({ view: "" as Cell["view"], widget_type: "" as Cell["widget_type"] }))).toBe("timeseries");
    expect(cellView(base({ view: undefined, widget_type: undefined as unknown as Cell["widget_type"] }))).toBe(
      "timeseries",
    );
  });

  it("passes a real but unknown view through unchanged (still an honest unsupported state)", () => {
    expect(cellView(base({ view: "ext:thecrew/scene" as Cell["view"] }))).toBe("ext:thecrew/scene");
    expect(cellView(base({ view: "totally-made-up" as Cell["view"] }))).toBe("totally-made-up");
  });
});
