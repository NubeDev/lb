// Pure unit tests for the canvas projection (visual-canvas-builder slice). The model is the single
// source of truth; `toFlow` re-derives the canvas from the typed query + schema + layout blob, and
// `joinFromConnect` / `layoutFromNodes` map view events back to typed edits. Node positions never
// appear in the SqlBuilderQuery (the model-as-truth invariant). No React, no jsdom.

import { describe, expect, it } from "vitest";

import type { SqlBuilderQuery, SqlJoin } from "@/lib/panel-kit/sql/query";
import type { Schema } from "@/lib/schema";
import {
  disconnectJoin,
  joinFromConnect,
  layoutFromNodes,
  removeTable,
  toFlow,
  type CanvasModel,
} from "./canvasModel";

const schema: Schema = {
  tables: [
    {
      name: "site",
      columns: [
        { name: "id", type: "string" },
        { name: "name", type: "string" },
      ],
    },
    {
      name: "point_reading",
      columns: [
        { name: "site_id", type: "string" },
        { name: "value", type: "float" },
      ],
    },
    {
      name: "kind",
      columns: [{ name: "id", type: "string" }],
    },
  ],
};

describe("canvasModel.toFlow", () => {
  it("derives 2 nodes + 1 edge from a single-join query (handles carry the ON columns)", () => {
    const q: SqlBuilderQuery = {
      table: "site",
      joins: [
        { table: "point_reading", type: "inner", on: [{ leftColumn: "id", rightColumn: "site_id" }] },
      ],
      columns: [],
      filters: [],
    };
    const view = toFlow(q, schema);
    expect(view.nodes.map((n) => n.id)).toEqual(["site", "point_reading"]);
    expect(view.nodes[0].data.columns.map((c) => c.name)).toEqual(["id", "name"]);
    expect(view.edges).toHaveLength(1);
    const e = view.edges[0];
    expect(e.source).toBe("site");
    expect(e.target).toBe("point_reading");
    expect(e.sourceHandle).toBe("id");
    expect(e.targetHandle).toBe("site_id");
    expect(e.data.joinType).toBe("inner");
  });

  it("a CROSS join with no `on` has a node but no edge (nothing to anchor handles to) and is NOT pending", () => {
    const q: SqlBuilderQuery = {
      table: "site",
      joins: [{ table: "kind", type: "cross" }],
      columns: [],
      filters: [],
    };
    const view = toFlow(q, schema);
    expect(view.nodes.map((n) => n.id)).toEqual(["site", "kind"]);
    // An edge with empty handle ids never renders in React Flow — a keyless join gets no edge.
    expect(view.edges).toHaveLength(0);
    expect(view.nodes[1].data.pending).toBe(false); // cross legitimately has no ON — it IS in the SQL
  });

  it("a canvas-added table (non-cross join, empty `on`) is a PENDING node with no edge", () => {
    const q: SqlBuilderQuery = {
      table: "site",
      joins: [{ table: "point_reading", type: "inner", on: [] }],
      columns: [],
      filters: [],
    };
    const view = toFlow(q, schema);
    expect(view.nodes.map((n) => n.id)).toEqual(["site", "point_reading"]);
    expect(view.nodes[0].data.pending).toBe(false); // the FROM table is never pending
    expect(view.nodes[1].data.pending).toBe(true);
    expect(view.nodes[1].data.columns.map((c) => c.name)).toEqual(["site_id", "value"]);
    expect(view.edges).toHaveLength(0);
  });

  it("returns no nodes when the query has no FROM table", () => {
    expect(toFlow({ table: "", columns: [], filters: [] }, schema)).toEqual({ nodes: [], edges: [] });
  });

  it("falls back to an auto-grid when no layout blob is supplied", () => {
    const q: SqlBuilderQuery = {
      table: "site",
      joins: [{ table: "point_reading", type: "inner", on: [{ leftColumn: "id", rightColumn: "site_id" }] }],
      columns: [],
      filters: [],
    };
    const view = toFlow(q, schema);
    expect(view.nodes[0].position).toEqual({ x: 0, y: 40 });
    expect(view.nodes[1].position).toEqual({ x: 280, y: 40 });
  });

  it("restores node positions from the persisted layout blob (keyed by table)", () => {
    const q: SqlBuilderQuery = {
      table: "site",
      joins: [{ table: "point_reading", type: "inner", on: [{ leftColumn: "id", rightColumn: "site_id" }] }],
      columns: [],
      filters: [],
    };
    const view = toFlow(q, schema, { site: { x: 10, y: 20 }, point_reading: { x: 300, y: 80 } });
    expect(view.nodes[0].position).toEqual({ x: 10, y: 20 });
    expect(view.nodes[1].position).toEqual({ x: 300, y: 80 });
  });

  it("resolves leftTable from a 2nd join's source handle (≥2 joins)", () => {
    // site → point_reading (on site.id), then sensor → reading.value (left side is point_reading).
    const q: SqlBuilderQuery = {
      table: "site",
      joins: [
        { table: "point_reading", type: "inner", on: [{ leftColumn: "id", rightColumn: "site_id" }] },
        {
          table: "sensor",
          type: "left",
          on: [{ leftTable: "point_reading", leftColumn: "id", rightColumn: "reading_id" }],
        },
      ],
      columns: [],
      filters: [],
    };
    const view = toFlow(q, schema);
    expect(view.edges[1].source).toBe("point_reading");
    expect(view.edges[1].data.joinType).toBe("left");
  });
});

describe("canvasModel.joinFromConnect", () => {
  it("builds an INNER join from two column handles (default type, no leftTable for FROM-source)", () => {
    const join = joinFromConnect(
      { table: "site", column: "id" },
      { table: "point_reading", column: "site_id" },
    );
    expect(join).toEqual({
      table: "point_reading",
      type: "inner",
      on: [{ leftTable: undefined, leftColumn: "id", rightColumn: "site_id" }],
    });
  });

  it("sets leftTable when the source table is NOT the FROM table (≥2 joins)", () => {
    const join = joinFromConnect(
      { table: "point_reading", column: "id" },
      { table: "sensor", column: "reading_id" },
      "left",
      "site",
    );
    expect(join.type).toBe("left");
    expect(join.on?.[0].leftTable).toBe("point_reading");
  });
});

describe("canvasModel round-trip", () => {
  it("query → toFlow → joinFromConnect → apply → toFlow yields a stable edge (model-as-truth)", () => {
    // Start: site only (no joins). The canvas shows one node.
    const q0: SqlBuilderQuery = { table: "site", columns: [], filters: [] };
    let view: CanvasModel = toFlow(q0, schema);
    expect(view.nodes).toHaveLength(1);
    expect(view.edges).toHaveLength(0);

    // Simulate a connect drag: site.id → point_reading.site_id (the user has just dropped table
    // `point_reading` and dragged its column, OR dragged site→point_reading directly).
    const newJoin: SqlJoin = joinFromConnect(
      { table: "site", column: "id" },
      { table: "point_reading", column: "site_id" },
    );
    const q1: SqlBuilderQuery = { ...q0, joins: [...(q0.joins ?? []), newJoin] };

    // Re-derive the canvas from the updated query.
    view = toFlow(q1, schema);
    expect(view.nodes.map((n) => n.id)).toEqual(["site", "point_reading"]);
    expect(view.edges).toHaveLength(1);
    expect(view.edges[0]).toMatchObject({
      source: "site",
      target: "point_reading",
      sourceHandle: "id",
      targetHandle: "site_id",
    });

    // Re-deriving AGAIN from the same query is stable (idempotent — no duplicate edges).
    expect(toFlow(q1, schema).edges).toHaveLength(1);

    // CRITICAL: the query carries NO node positions (the model-as-truth invariant — positions are
    // view state, persisted separately as the opaque builderLayout blob).
    expect((q1 as unknown as { position?: unknown }).position).toBeUndefined();
    expect(JSON.stringify(q1)).not.toContain('"position"');
    expect(JSON.stringify(q1)).not.toContain('"x"');
  });
});

describe("canvasModel typed edits (disconnect / remove-table)", () => {
  const joined: SqlBuilderQuery = {
    table: "site",
    joins: [{ table: "point_reading", type: "left", on: [{ leftColumn: "id", rightColumn: "site_id" }] }],
    columns: [
      { name: "name", table: "site" },
      { name: "value", table: "point_reading", aggregation: "avg" },
    ],
    filters: [
      { column: "name", table: "site", operator: "=", value: "hq" },
      { column: "value", table: "point_reading", operator: ">", value: 1 },
    ],
    groupBy: ["name", { table: "point_reading", column: "value" }],
    orderBy: [
      { column: "name", table: "site", direction: "asc" },
      { column: "value", table: "point_reading", direction: "desc" },
    ],
  };

  it("disconnectJoin clears the join's ON keys (table becomes pending, stays on canvas)", () => {
    const q = disconnectJoin(joined, "point_reading");
    expect(q.joins).toEqual([{ table: "point_reading", type: "left", on: [] }]);
    const view = toFlow(q, schema);
    expect(view.nodes.map((n) => n.id)).toEqual(["site", "point_reading"]);
    expect(view.nodes[1].data.pending).toBe(true);
    expect(view.edges).toHaveLength(0);
    // Columns/filters are kept — re-wiring the join restores them into the SQL.
    expect(q.columns).toHaveLength(2);
  });

  it("removeTable drops the join AND every column/filter/sort/group-by referencing it", () => {
    const q = removeTable(joined, "point_reading");
    expect(q.joins).toEqual([]);
    expect(q.columns).toEqual([{ name: "name", table: "site" }]);
    expect(q.filters).toEqual([{ column: "name", table: "site", operator: "=", value: "hq" }]);
    expect(q.groupBy).toEqual(["name"]);
    expect(q.orderBy).toEqual([{ column: "name", table: "site", direction: "asc" }]);
  });

  it("removeTable handles the legacy single-object orderBy shape", () => {
    const q = removeTable(
      { ...joined, orderBy: { column: "value", table: "point_reading", direction: "desc" } },
      "point_reading",
    );
    expect(q.orderBy).toEqual([]);
  });
});

describe("canvasModel.layoutFromNodes", () => {
  it("round-trips node positions into the opaque layout blob", () => {
    const q: SqlBuilderQuery = {
      table: "site",
      joins: [{ table: "point_reading", type: "inner", on: [{ leftColumn: "id", rightColumn: "site_id" }] }],
      columns: [],
      filters: [],
    };
    const view = toFlow(q, schema, { site: { x: 11, y: 22 }, point_reading: { x: 33, y: 44 } });
    const blob = layoutFromNodes(view.nodes);
    expect(blob).toEqual({ site: { x: 11, y: 22 }, point_reading: { x: 33, y: 44 } });
    // Re-feeding the blob reproduces the same positions.
    expect(toFlow(q, schema, blob).nodes.map((n) => n.position)).toEqual([
      { x: 11, y: 22 },
      { x: 33, y: 44 },
    ]);
  });
});
