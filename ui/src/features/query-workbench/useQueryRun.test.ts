// Pure-logic unit tests for the query-run dispatch (query-workbench-view scope, slice 3). The
// `runKindFor` decision is the only transport-free logic in `useQueryRun` — the rest is the real
// run path, exercised end-to-end by `QueryWorkbench.gateway.test.tsx` against the real gateway
// (rule 9: no mocks for the run path). This file pins the dispatch decision + the sentinel.

import { describe, expect, it } from "vitest";

import { SURREAL_LOCAL, runKindFor } from "./useQueryRun";

describe("runKindFor — the engine dispatch", () => {
  it(`classifies the "${SURREAL_LOCAL}" sentinel as the surreal dialect (store.query)`, () => {
    expect(runKindFor(SURREAL_LOCAL)).toBe("surreal");
  });

  it("classifies any other source string as the federation path (federation.query)", () => {
    expect(runKindFor("demo-buildings")).toBe("federation");
    expect(runKindFor("timescale")).toBe("federation");
    expect(runKindFor("")).toBe("federation");
  });

  it("is stable across the two classifications (no third branch — rule 2: two engines only)", () => {
    const inputs = [SURREAL_LOCAL, "demo-buildings", "timescale", "", "any-name"];
    const kinds = new Set(inputs.map(runKindFor));
    expect(kinds).toEqual(new Set(["surreal", "federation"]));
  });
});

describe("SURREAL_LOCAL sentinel", () => {
  it("is the documented literal the workbench + DataView pin the Data-page mount to", () => {
    expect(SURREAL_LOCAL).toBe("surreal-local");
  });
});
