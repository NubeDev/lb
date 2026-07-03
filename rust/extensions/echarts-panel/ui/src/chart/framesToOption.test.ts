// Unit tests for the PURE `framesToOption` mapping. No ECharts instance, no DOM — just the option shape,
// proving the Field-tab options actually drive the chart (unit/decimals/thresholds/drawStyle/legend) and
// that degenerate input maps to an honest empty option (no fabricated series).

import { describe, it, expect } from "vitest";

import { framesToOption } from "./framesToOption";
import type { Frame, FieldConfig } from "./frame.types";

const tsFrame: Frame = {
  refId: "A",
  fields: [
    { name: "time", type: "time", values: [1, 2, 3] },
    { name: "value", type: "number", values: [10, 20, 30] },
  ],
};

describe("framesToOption", () => {
  it("maps one line series per numeric field with the time field as the x axis", () => {
    const opt = framesToOption([tsFrame]);
    const series = opt.series as Array<{ type: string; name: string; data: unknown[] }>;
    expect(series).toHaveLength(1);
    expect(series[0].type).toBe("line");
    expect(series[0].name).toBe("value");
    expect(series[0].data).toEqual([10, 20, 30]);
    expect((opt.xAxis as { data: unknown[] }).data).toEqual(["1", "2", "3"]);
  });

  it("honours custom.drawStyle = bar", () => {
    const fc: FieldConfig = { defaults: { custom: { drawStyle: "bar" } } };
    const opt = framesToOption([tsFrame], fc);
    const series = opt.series as Array<{ type: string }>;
    expect(series[0].type).toBe("bar");
  });

  it("renders thresholds as y-axis markLines", () => {
    const fc: FieldConfig = {
      defaults: { thresholds: { mode: "absolute", steps: [{ value: null, color: "green" }, { value: 25, color: "red" }] } },
    };
    const opt = framesToOption([tsFrame], fc);
    const series = opt.series as Array<{ markLine?: { data: Array<{ yAxis: number }> } }>;
    expect(series[0].markLine?.data).toEqual([{ yAxis: 25, lineStyle: { color: "red" } }]);
  });

  it("applies unit/decimals to the y-axis label formatter", () => {
    const fc: FieldConfig = { defaults: { unit: "°C", decimals: 1 } };
    const opt = framesToOption([tsFrame], fc);
    const fmt = (opt.yAxis as { axisLabel: { formatter: (v: number) => string } }).axisLabel.formatter;
    expect(fmt(12.34)).toBe("12.3 °C");
  });

  it("shows a legend when more than one series", () => {
    const two: Frame = {
      fields: [
        { name: "time", type: "time", values: [1, 2] },
        { name: "a", type: "number", values: [1, 2] },
        { name: "b", type: "number", values: [3, 4] },
      ],
    };
    const opt = framesToOption([two]);
    expect((opt.legend as { show: boolean }).show).toBe(true);
    expect((opt.series as unknown[]).length).toBe(2);
  });

  it("maps empty frames to an option with no series (honest empty)", () => {
    const opt = framesToOption([]);
    expect(opt.series).toEqual([]);
  });

  it("does not plot string fields as a fake series", () => {
    const withLabel: Frame = {
      fields: [
        { name: "time", type: "time", values: [1, 2] },
        { name: "label", type: "string", values: ["x", "y"] },
        { name: "value", type: "number", values: [5, 6] },
      ],
    };
    const opt = framesToOption([withLabel]);
    const series = opt.series as Array<{ name: string }>;
    expect(series).toHaveLength(1);
    expect(series[0].name).toBe("value");
  });
});
