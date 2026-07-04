// Unit tests for the page-context builder (agent-dock scope) — tenant-stripped surface/path/search.

import { describe, expect, it } from "vitest";

import { buildPageContext } from "./pageContext";

describe("buildPageContext", () => {
  it("derives surface, tenant-stripped path, and typed search", () => {
    const ctx = buildPageContext("/t/acme/dashboards", { d: "sales", from: "now-24h" });
    expect(ctx.surface).toBe("dashboards");
    expect(ctx.path).toBe("/dashboards");
    expect(ctx.search).toEqual({ d: "sales", from: "now-24h" });
  });

  it("coerces non-string search values and drops null/undefined", () => {
    const ctx = buildPageContext("/t/acme/flows", {
      n: 3,
      on: true,
      obj: { a: 1 },
      gone: null,
      missing: undefined,
    });
    expect(ctx.search).toEqual({ n: "3", on: "true", obj: '{"a":1}' });
  });

  it("falls back to the channels surface for an unknown path (never throws)", () => {
    const ctx = buildPageContext("/t/acme/nowhere", {});
    expect(ctx.surface).toBe("channels");
    expect(ctx.path).toBe("/nowhere");
  });
});
