// Pure-logic unit tests for the SQL query builder (widget-builder Slice C): the `toSurrealQL` renderer
// (columns, aggregation, filter, group-by, order, limit) and the Builder→Code→Builder round-trip of a
// builder-authored query. No gateway — the real round-trip into rendered widgets lives in
// sqlSource.gateway.test.tsx. These cover the scope's mandatory `toSurrealQL` unit cases + the
// round-trip decision deterministically.

import { describe, expect, it } from "vitest";

import { emptyQuery, type SqlBuilderQuery } from "./query";
import { toSurrealQL } from "./toSurrealQL";

describe("toSurrealQL", () => {
  it("renders a bare SELECT * when no columns are chosen", () => {
    expect(toSurrealQL(emptyQuery("series"))).toBe("SELECT * FROM series");
  });

  it("returns empty string when no table is chosen (an incomplete builder)", () => {
    expect(toSurrealQL(emptyQuery(""))).toBe("");
  });

  it("renders explicit columns", () => {
    const q: SqlBuilderQuery = { ...emptyQuery("series"), columns: [{ name: "seq" }, { name: "payload" }] };
    expect(toSurrealQL(q)).toBe("SELECT seq, payload FROM series");
  });

  it("renders aggregations with aliases", () => {
    const q: SqlBuilderQuery = {
      ...emptyQuery("series"),
      columns: [
        { name: "*", aggregation: "count" },
        { name: "payload", aggregation: "avg" },
      ],
    };
    expect(toSurrealQL(q)).toBe("SELECT count() AS count, math::avg(payload) AS avg_payload FROM series");
  });

  it("renders a WHERE filter, quoting string values and passing numbers through", () => {
    const q: SqlBuilderQuery = {
      ...emptyQuery("series"),
      filters: [
        { column: "series", operator: "=", value: "cpu" },
        { column: "seq", operator: ">", value: 3 },
      ],
    };
    expect(toSurrealQL(q)).toBe("SELECT * FROM series WHERE series = 'cpu' AND seq > 3");
  });

  it("escapes single quotes in a string filter value (no literal break-out)", () => {
    const q: SqlBuilderQuery = {
      ...emptyQuery("series"),
      filters: [{ column: "name", operator: "=", value: "o'brien" }],
    };
    expect(toSurrealQL(q)).toBe("SELECT * FROM series WHERE name = 'o''brien'");
  });

  it("renders GROUP BY, ORDER BY, and LIMIT", () => {
    const q: SqlBuilderQuery = {
      ...emptyQuery("series"),
      columns: [{ name: "series" }, { name: "payload", aggregation: "sum" }],
      groupBy: ["series"],
      orderBy: { column: "series", direction: "desc" },
      limit: 50,
    };
    expect(toSurrealQL(q)).toBe(
      "SELECT series, math::sum(payload) AS sum_payload FROM series GROUP BY series ORDER BY series DESC LIMIT 50",
    );
  });

  // ── visual-canvas-builder slice additions (HAVING, AND/OR, LIKE, IS NULL, multi-sort, count_distinct) ──

  it("emits a HAVING clause with the aggregate expression (not the alias)", () => {
    const q: SqlBuilderQuery = {
      table: "series",
      columns: [{ name: "payload", aggregation: "avg", alias: "avg_p" }],
      filters: [
        { column: "series", operator: "=", value: "cpu" },
        { column: "payload", operator: ">", value: 10, isAggregate: true, aggregation: "avg" },
      ],
      groupBy: ["series"],
    };
    expect(toSurrealQL(q)).toBe(
      "SELECT math::avg(payload) AS avg_p FROM series WHERE series = 'cpu' GROUP BY series HAVING math::avg(payload) > 10",
    );
  });

  it("chains filters with AND/OR by each filter's `logical`", () => {
    const q: SqlBuilderQuery = {
      table: "t",
      columns: [],
      filters: [
        { column: "a", operator: "=", value: 1 },
        { column: "b", operator: ">", value: 2, logical: "OR" },
      ],
    };
    expect(toSurrealQL(q)).toBe("SELECT * FROM t WHERE a = 1 OR b > 2");
  });

  it("renders LIKE / IS NULL / IS NOT NULL", () => {
    const q: SqlBuilderQuery = {
      table: "t",
      columns: [],
      filters: [
        { column: "name", operator: "LIKE", value: "cpu%" },
        { column: "ts", operator: "IS NULL", logical: "AND" },
      ],
    };
    expect(toSurrealQL(q)).toBe("SELECT * FROM t WHERE name LIKE 'cpu%' AND ts IS NULL");
  });

  it("renders multi-column ORDER BY (bare identifiers, comma-separated)", () => {
    const q: SqlBuilderQuery = {
      table: "t",
      columns: [],
      filters: [],
      orderBy: [
        { column: "a", direction: "asc" },
        { column: "b", direction: "desc" },
      ],
    };
    expect(toSurrealQL(q)).toBe("SELECT * FROM t ORDER BY a ASC, b DESC");
  });

  it("renders count_distinct as count(DISTINCT col)", () => {
    const q: SqlBuilderQuery = {
      table: "t",
      columns: [{ name: "c", aggregation: "count_distinct" }],
      filters: [],
    };
    expect(toSurrealQL(q)).toBe("SELECT count(DISTINCT c) AS count_distinct_c FROM t");
  });

  it("defensively drops joins (SurrealQL has no ANSI JOIN ON — never emit invalid SurrealQL)", () => {
    const q: SqlBuilderQuery = {
      table: "t",
      joins: [{ table: "u", type: "inner", on: [{ leftColumn: "id", rightColumn: "t_id" }] }],
      columns: [],
      filters: [],
    };
    expect(toSurrealQL(q)).toBe("SELECT * FROM t");
  });
});

describe("Builder → Code → Builder round-trip", () => {
  it("a builder-authored query regenerates the same SurrealQL after a round-trip", () => {
    // Author in Builder mode: this is the typed query the VisualEditor holds.
    const builder: SqlBuilderQuery = {
      table: "series",
      columns: [{ name: "seq" }, { name: "payload", aggregation: "max" }],
      filters: [{ column: "seq", operator: ">=", value: 2 }],
      groupBy: ["seq"],
      orderBy: { column: "seq", direction: "asc" },
      limit: 100,
    };

    // Builder→Code: the SQL source state stores BOTH the builder query AND the generated raw string.
    const rawSql = toSurrealQL(builder);
    expect(rawSql).toContain("SELECT seq, math::max(payload) AS max_payload FROM series");
    expect(rawSql).toContain("WHERE seq >= 2");
    expect(rawSql).toContain("GROUP BY seq");
    expect(rawSql).toContain("ORDER BY seq ASC");
    expect(rawSql).toContain("LIMIT 100");

    // Code→Builder (confirmed): we keep the stored builder query, and regenerating from it produces the
    // SAME string — the round-trip is stable for a builder-authored query.
    const regenerated = toSurrealQL(builder);
    expect(regenerated).toBe(rawSql);
  });
});
