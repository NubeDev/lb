import { describe, it, expect } from "vitest";
import { parseChoices, loadSchemaChoices, choicesFor, withChoices } from "./choices";

describe("parseChoices", () => {
  it("parses label:code pairs", () => {
    expect(parseChoices("low:0,medium:1,high:2")).toEqual([
      { code: 0, label: "low" },
      { code: 1, label: "medium" },
      { code: 2, label: "high" },
    ]);
  });
  it("returns undefined for empty / garbage", () => {
    expect(parseChoices("")).toBeUndefined();
    expect(parseChoices(undefined)).toBeUndefined();
    expect(parseChoices("nocode")).toBeUndefined();
  });
});

describe("schema choices index", () => {
  it("indexes by full type → prop and folds into aliases", () => {
    loadSchemaChoices([
      {
        vendor: "NubeIO",
        name: "alarm",
        components: [{ name: "limitAlarm", properties: [{ name: "severity", choices: "low:0,medium:1,high:2" }] }],
      },
    ]);
    const t = "NubeIO-alarm::limitAlarm";
    expect(choicesFor(t, "severity")).toHaveLength(3);
    expect(choicesFor(t, "missing")).toBeUndefined();

    // withChoices injects when the facet has no aliases…
    expect(withChoices(undefined, t, "severity")?.aliases).toHaveLength(3);
    // …but a facet's own aliases win.
    expect(withChoices({ aliases: [{ code: 9, label: "x" }] }, t, "severity")?.aliases).toHaveLength(1);
  });
});
