import { describe, expect, it } from "vitest";

import { buildPlot } from "./buildPlot";
import type { PlotSpec } from "./plotSpec";

describe("buildPlot", () => {
  it("builds a wide multi-series frame (one key per y field)", () => {
    const rows = [
      { t: "a", cpu: 1, mem: 10 },
      { t: "b", cpu: 2, mem: 20 },
    ];
    const spec: PlotSpec = { type: "line", xField: "t", yFields: ["cpu", "mem"] };
    const frame = buildPlot(rows, spec);
    expect(frame.series.map((s) => s.key)).toEqual(["cpu", "mem"]);
    expect(frame.data).toEqual([
      { __x: "a", cpu: 1, mem: 10 },
      { __x: "b", cpu: 2, mem: 20 },
    ]);
  });

  it("coerces numeric strings and nulls non-numbers rather than fabricating zero", () => {
    const rows = [{ t: "a", v: "3.5" }, { t: "b", v: "nope" }];
    const frame = buildPlot(rows, { type: "line", xField: "t", yFields: ["v"] });
    expect(frame.data[0].v).toBe(3.5);
    expect(frame.data[1].v).toBeNull();
  });

  it("pivots a long frame into one series per seriesField value", () => {
    const rows = [
      { t: "a", host: "x", cpu: 1 },
      { t: "a", host: "y", cpu: 2 },
      { t: "b", host: "x", cpu: 3 },
    ];
    const spec: PlotSpec = { type: "line", xField: "t", yFields: ["cpu"], seriesField: "host" };
    const frame = buildPlot(rows, spec);
    expect(frame.series.map((s) => s.name).sort()).toEqual(["x", "y"]);
    expect(frame.data).toContainEqual({ __x: "a", x: 1, y: 2 });
    expect(frame.data).toContainEqual({ __x: "b", x: 3 });
  });

  it("aggregates a pie by summing the y field per category", () => {
    const rows = [
      { k: "a", v: 1 },
      { k: "a", v: 2 },
      { k: "b", v: 5 },
    ];
    const frame = buildPlot(rows, { type: "pie", xField: "k", yFields: ["v"] });
    expect(frame.data).toEqual([
      { __x: "a", value: 3 },
      { __x: "b", value: 5 },
    ]);
  });

  it("bins a histogram into the requested bucket count", () => {
    const rows = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9].map((v) => ({ v }));
    const frame = buildPlot(rows, { type: "histogram", xField: "", yFields: ["v"], bins: 5 });
    expect(frame.data).toHaveLength(5);
    const total = frame.data.reduce((a, d) => a + (d.count as number), 0);
    expect(total).toBe(10);
  });
});
