// Unit tests for the schema → React Flow projection (datasources-ux ERD scope). Pure: tables → nodes,
// naming-convention edge inference (`<table>_id` / `<table>Ref`), no-edge cases, and empty schema →
// empty diagram. The fixture mirrors the timescale (postgres) datasource the ERD is eyeballed against:
// site / point / point_reading / point_tag / meter / meter_tag / site_tag with siteRef/pointRef-style
// ref columns.

import { describe, expect, it } from "vitest";

import { inferRelations, schemaToFlow } from "./schemaToFlow";
import type { ErdTable } from "./schemaToFlow";

/** Build a table fixture with minimal column noise (every column nullable, text type — only names
 *  matter for projection + inference). */
function table(name: string, cols: string[]): ErdTable {
  return {
    name,
    columns: cols.map((c) => ({ name: c, dataType: "text", nullable: true })),
  };
}

// The timescale-shaped fixture: site is the root; point refs site via `siteRef`; the reading + tag
// tables ref point via `pointRef`; meter is an island root; meter_tag refs meter via `meter_id`;
// site_tag refs site via `siteRef`.
const TIMELSCALE: ErdTable[] = [
  table("site", ["id", "name"]),
  table("point", ["id", "siteRef", "name"]),
  table("point_reading", ["id", "pointRef", "value"]),
  table("point_tag", ["id", "pointRef", "tag"]),
  table("meter", ["id", "name"]),
  table("meter_tag", ["id", "meter_id", "tag"]),
  table("site_tag", ["id", "siteRef", "tag"]),
];

describe("schemaToFlow — tables → nodes", () => {
  it("projects one node per table, each carrying its name + columns", () => {
    const { nodes } = schemaToFlow(TIMELSCALE);
    expect(nodes).toHaveLength(7);
    expect(nodes.map((n) => n.id).sort()).toEqual(
      ["meter", "meter_tag", "point", "point_reading", "point_tag", "site", "site_tag"],
    );
    const point = nodes.find((n) => n.id === "point")!;
    expect(point.data.name).toBe("point");
    expect(point.data.columns.map((c) => c.name)).toEqual(["id", "siteRef", "name"]);
    expect(point.type).toBe("schemaTable");
  });

  it("leaves positions at {0,0} — layout is erdLayout's job, not the projection's", () => {
    const { nodes } = schemaToFlow(TIMELSCALE);
    for (const n of nodes) expect(n.position).toEqual({ x: 0, y: 0 });
  });
});

describe("schemaToFlow — convention-based edge inference", () => {
  it("infers a `Ref`-suffix edge to the named table, attached to the parent's `id` handle", () => {
    const { relations } = schemaToFlow(TIMELSCALE);
    const pointToSite = relations.find(
      (r) => r.source === "point" && r.sourceHandle === "siteRef",
    );
    expect(pointToSite).toEqual({
      source: "point",
      sourceHandle: "siteRef",
      target: "site",
      targetHandle: "id",
      reason: "suffix Ref",
    });
  });

  it("infers an `_id`-suffix edge to the named table (meter_id → meter)", () => {
    const { relations } = schemaToFlow(TIMELSCALE);
    const tagToMeter = relations.find((r) => r.source === "meter_tag");
    expect(tagToMeter).toMatchObject({
      source: "meter_tag",
      sourceHandle: "meter_id",
      target: "meter",
      targetHandle: "id",
      reason: "suffix _id",
    });
  });

  it("resolves the parent case-insensitively (siteRef → site) and infers ALL ref edges", () => {
    const { relations } = schemaToFlow(TIMELSCALE);
    const targets = relations.map((r) => `${r.source}.${r.sourceHandle}->${r.target}`).sort();
    expect(targets).toEqual(
      [
        "meter_tag.meter_id->meter",
        "point.siteRef->site",
        "point_reading.pointRef->point",
        "point_tag.pointRef->point",
        "site_tag.siteRef->site",
      ].sort(),
    );
  });

  it("never fabricates an edge when no table matches the ref column", () => {
    const { relations, edges } = schemaToFlow([
      table("order", ["id", "customer_id", "coupon_code"]),
      // No `customer` table — `customer_id` must NOT produce an edge or a phantom node.
    ]);
    expect(relations).toEqual([]);
    expect(edges).toEqual([]);
  });

  it("does not treat the identity `id` column as a ref", () => {
    const { relations } = schemaToFlow([table("thing", ["id", "name"])]);
    expect(relations).toEqual([]);
  });

  it("attaches the edge at node level (targetHandle undefined) when the parent has no `id` column", () => {
    const { relations } = schemaToFlow([
      table("foo", ["bar_id", "label"]),
      table("bar", ["label"]), // parent exists but owns no `id` column
    ]);
    expect(relations).toEqual([
      {
        source: "foo",
        sourceHandle: "bar_id",
        target: "bar",
        targetHandle: undefined,
        reason: "suffix _id",
      },
    ]);
  });

  it("marks every inferred edge dashed + `inferred` so the canvas never looks like declared FKs", () => {
    const { edges } = schemaToFlow(TIMELSCALE);
    expect(edges.length).toBeGreaterThan(0);
    for (const e of edges) {
      expect(e.style?.strokeDasharray).toBeTruthy();
      expect((e.data as { inferred?: boolean }).inferred).toBe(true);
    }
  });

  it("exposes inferRelations on its own (pure, no nodes/edges build)", () => {
    expect(inferRelations([table("a", ["b_id"]), table("b", ["id"])])).toEqual([
      expect.objectContaining({ source: "a", sourceHandle: "b_id", target: "b", targetHandle: "id" }),
    ]);
  });
});

describe("schemaToFlow — empty schema", () => {
  it("returns an empty diagram (no nodes, no edges, no relations)", () => {
    const { nodes, edges, relations } = schemaToFlow([]);
    expect(nodes).toEqual([]);
    expect(edges).toEqual([]);
    expect(relations).toEqual([]);
  });
});
