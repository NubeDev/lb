import { describe, expect, it } from "vitest";

import { suggestPlot } from "./suggestPlot";

describe("suggestPlot", () => {
  it("suggests a line over a temporal x", () => {
    const rows = [
      { t: "2026-01-01T00:00:00Z", cpu: 1 },
      { t: "2026-01-01T00:01:00Z", cpu: 2 },
    ];
    expect(suggestPlot(rows)).toEqual({ type: "line", xField: "t", yFields: ["cpu"], smooth: true });
  });

  it("suggests a bar for a categorical x", () => {
    const rows = [
      { host: "a", cpu: 1 },
      { host: "b", cpu: 2 },
    ];
    expect(suggestPlot(rows)).toEqual({ type: "bar", xField: "host", yFields: ["cpu"] });
  });

  it("suggests a histogram for a lone numeric column with enough rows", () => {
    const rows = [1, 2, 3, 4, 5].map((v) => ({ v }));
    const spec = suggestPlot(rows);
    expect(spec?.type).toBe("histogram");
    expect(spec?.yFields).toEqual(["v"]);
  });

  it("returns null when there is nothing numeric to plot (table-only)", () => {
    expect(suggestPlot([{ a: "x", b: "y" }])).toBeNull();
  });
});
