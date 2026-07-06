// Unit tests for the ERD deterministic auto-layout (datasources-ux ERD scope). Pure: same input → same
// positions (no wall-clock/random), parents land left of their referrers, and within-column stacking is
// alphabetical. The fixture mirrors the timescale (postgres) datasource: site ← point ← point_reading.

import { describe, expect, it } from "vitest";

import { layoutErd } from "./erdLayout";
import { schemaToFlow } from "./schemaToFlow";
import type { ErdTable } from "./schemaToFlow";

function table(name: string, cols: string[]): ErdTable {
  return {
    name,
    columns: cols.map((c) => ({ name: c, dataType: "text", nullable: true })),
  };
}

const CHAIN: ErdTable[] = [
  table("site", ["id", "name"]),
  table("point", ["id", "siteRef"]),
  table("point_reading", ["id", "pointRef"]),
];

describe("erdLayout — empty + determinism", () => {
  it("returns the empty set unchanged", () => {
    expect(layoutErd([], [])).toEqual([]);
  });

  it("is deterministic — the same nodes+edges yield identical positions across calls", () => {
    const flow = schemaToFlow(CHAIN);
    const a = layoutErd(flow.nodes, flow.edges).map((n) => [n.id, n.position]);
    const b = layoutErd(schemaToFlow(CHAIN).nodes, schemaToFlow(CHAIN).edges).map(
      (n) => [n.id, n.position],
    );
    expect(a).toEqual(b);
  });
});

describe("erdLayout — layered column placement", () => {
  it("places a parent LEFT of its referrer (depth grows to the right along the FK chain)", () => {
    const flow = schemaToFlow(CHAIN);
    const laid = layoutErd(flow.nodes, flow.edges);
    const pos = new Map(laid.map((n) => [n.id, n.position]));
    // site (root) ← point ← point_reading: each child one column to the right of its parent.
    expect(pos.get("site")!.x).toBeLessThan(pos.get("point")!.x);
    expect(pos.get("point")!.x).toBeLessThan(pos.get("point_reading")!.x);
  });

  it("stacks same-column nodes alphabetically (two roots land at depth 0, ordered by name)", () => {
    const flow = schemaToFlow([
      table("site", ["id"]),
      table("meter", ["id"]), // island root — same depth-0 column as site
      table("point", ["id", "siteRef"]),
    ]);
    const laid = layoutErd(flow.nodes, flow.edges);
    const yById = new Map(laid.map((n) => [n.id, n.position.y]));
    // Alphabetical top-to-bottom: meter (m) above site (s). Assert by id — the array order is the
    // original input order, not the within-column stack order.
    expect(yById.get("meter")).toBe(0);
    expect(yById.get("site")).toBe(240);
    expect(yById.get("point")).toBe(0); // alone in its column
  });

  it("places a table referenced by several children at the smallest depth (root-left)", () => {
    const flow = schemaToFlow([
      table("site", ["id"]),
      table("point", ["id", "siteRef"]),
      table("site_tag", ["id", "siteRef"]), // both children → site
    ]);
    const laid = layoutErd(flow.nodes, flow.edges);
    const pos = new Map(laid.map((n) => [n.id, n.position]));
    expect(pos.get("site")!.x).toBe(0); // root, depth 0
    expect(pos.get("point")!.x).toBeGreaterThan(0);
    expect(pos.get("site_tag")!.x).toBeGreaterThan(0);
  });

  it("does not mutate the input node positions", () => {
    const flow = schemaToFlow(CHAIN);
    const before = flow.nodes.map((n) => ({ id: n.id, x: n.position.x }));
    layoutErd(flow.nodes, flow.edges);
    const after = flow.nodes.map((n) => ({ id: n.id, x: n.position.x }));
    expect(after).toEqual(before); // original {0,0} untouched — layout returns a new array
  });
});
