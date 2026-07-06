// Pure-logic unit tests for the standard-SQL emitter (query-builder-common scope) — the federation
// analog of `toSurrealQL.test.ts`. Goldens covering the dialect deltas the scope names: identifier
// double-quoting (vs Surreal's bare), ANSI aggregate spelling (`SUM("col")` vs `math::sum`), filter
// value escaping, the empty/table-less guard, and LIMIT. No gateway — the end-to-end builder→rows
// proof against a real sqlite engine lives in `federation_sqlite_test.rs`; this file pins the
// string the builder produces.

import { describe, expect, it } from "vitest";

import { emptyQuery, type SqlBuilderQuery } from "./query";
import { toStandardSql } from "./toStandardSql";

describe("toStandardSql", () => {
  it("renders a bare SELECT * when no columns are chosen (table name quoted)", () => {
    expect(toStandardSql(emptyQuery("point_reading"))).toBe('SELECT * FROM "point_reading"');
  });

  it("returns empty string when no table is chosen (an incomplete builder)", () => {
    expect(toStandardSql(emptyQuery(""))).toBe("");
  });

  it("renders explicit columns, each double-quoted", () => {
    const q: SqlBuilderQuery = {
      ...emptyQuery("point_reading"),
      columns: [{ name: "point_id" }, { name: "value" }],
    };
    expect(toStandardSql(q)).toBe('SELECT "point_id", "value" FROM "point_reading"');
  });

  it("renders ANSI aggregations with aliases (COUNT(*), AVG)", () => {
    const q: SqlBuilderQuery = {
      ...emptyQuery("point_reading"),
      columns: [
        { name: "*", aggregation: "count" },
        { name: "value", aggregation: "avg" },
      ],
    };
    // Aliases are identifiers too — double-quoted (the safe superset; `count` is reserved in some
    // dialects). The engine returns lowercase keys the table/chart views map onto.
    expect(toStandardSql(q)).toBe(
      'SELECT COUNT(*) AS "count", AVG("value") AS "avg_value" FROM "point_reading"',
    );
  });

  it("renders SUM/MIN/MAX aggregations", () => {
    const q: SqlBuilderQuery = {
      ...emptyQuery("reading"),
      columns: [
        { name: "value", aggregation: "sum" },
        { name: "value", aggregation: "min" },
        { name: "value", aggregation: "max" },
      ],
    };
    expect(toStandardSql(q)).toBe(
      'SELECT SUM("value") AS "sum_value", MIN("value") AS "min_value", MAX("value") AS "max_value" FROM "reading"',
    );
  });

  it("renders COUNT(col) (not COUNT(*)) when a column is named", () => {
    const q: SqlBuilderQuery = {
      ...emptyQuery("reading"),
      columns: [{ name: "value", aggregation: "count" }],
    };
    expect(toStandardSql(q)).toBe('SELECT COUNT("value") AS "count_value" FROM "reading"');
  });

  it("renders a WHERE filter, quoting string values and passing numbers through", () => {
    const q: SqlBuilderQuery = {
      ...emptyQuery("point_reading"),
      filters: [
        { column: "point_id", operator: "=", value: "p1" },
        { column: "value", operator: ">", value: 3 },
      ],
    };
    expect(toStandardSql(q)).toBe(
      'SELECT * FROM "point_reading" WHERE "point_id" = \'p1\' AND "value" > 3',
    );
  });

  it("escapes single quotes in a string filter value (no literal break-out)", () => {
    const q: SqlBuilderQuery = {
      ...emptyQuery("site"),
      filters: [{ column: "name", operator: "=", value: "o'brien" }],
    };
    expect(toStandardSql(q)).toBe('SELECT * FROM "site" WHERE "name" = \'o\'\'brien\'');
  });

  it("escapes embedded double-quotes in a column name (no identifier break-out)", () => {
    const q: SqlBuilderQuery = {
      ...emptyQuery('weird " table'),
      columns: [{ name: 'col " name' }],
    };
    expect(toStandardSql(q)).toBe('SELECT "col "" name" FROM "weird "" table"');
  });

  it("renders GROUP BY, ORDER BY, and LIMIT (columns double-quoted)", () => {
    const q: SqlBuilderQuery = {
      ...emptyQuery("point_reading"),
      columns: [{ name: "point_id" }, { name: "value", aggregation: "sum" }],
      groupBy: ["point_id"],
      orderBy: { column: "point_id", direction: "desc" },
      limit: 50,
    };
    expect(toStandardSql(q)).toBe(
      'SELECT "point_id", SUM("value") AS "sum_value" FROM "point_reading" GROUP BY "point_id" ORDER BY "point_id" DESC LIMIT 50',
    );
  });

  it("does not emit LIMIT 0 / negative / non-integer (the host caps regardless)", () => {
    const q: SqlBuilderQuery = { ...emptyQuery("t"), limit: 0 };
    expect(toStandardSql(q)).toBe('SELECT * FROM "t"');
    const q2: SqlBuilderQuery = { ...emptyQuery("t"), limit: -5 as unknown as number };
    expect(toStandardSql(q2)).toBe('SELECT * FROM "t"');
  });

  // ── visual-canvas-builder slice additions (joins, HAVING, aliases, multi-sort, OR, LIKE, IS NULL) ──

  it("renders a single INNER join (qualified identifiers, ON clause)", () => {
    const q: SqlBuilderQuery = {
      table: "site",
      joins: [{ table: "point_reading", type: "inner", on: [{ leftColumn: "id", rightColumn: "site_id" }] }],
      columns: [{ name: "name", table: "site" }, { name: "value", table: "point_reading" }],
      filters: [],
    };
    expect(toStandardSql(q)).toBe(
      'SELECT "site"."name", "point_reading"."value" FROM "site" INNER JOIN "point_reading" ON "site"."id" = "point_reading"."site_id"',
    );
  });

  it("renders LEFT / RIGHT / FULL joins with the uppercase type, and CROSS with no ON", () => {
    const base = (type: "left" | "right" | "full" | "cross") => ({
      table: "a",
      joins: [{ table: "b", type, on: [{ leftColumn: "x", rightColumn: "y" }] }],
      columns: [],
      filters: [],
    });
    expect(toStandardSql(base("left"))).toBe('SELECT * FROM "a" LEFT JOIN "b" ON "a"."x" = "b"."y"');
    expect(toStandardSql(base("right"))).toBe('SELECT * FROM "a" RIGHT JOIN "b" ON "a"."x" = "b"."y"');
    expect(toStandardSql(base("full"))).toBe('SELECT * FROM "a" FULL JOIN "b" ON "a"."x" = "b"."y"');
    expect(toStandardSql(base("cross"))).toBe('SELECT * FROM "a" CROSS JOIN "b"');
  });

  it("renders a composite (multi-key) ON joined by AND", () => {
    const q: SqlBuilderQuery = {
      table: "a",
      joins: [
        {
          table: "b",
          type: "inner",
          on: [
            { leftColumn: "x", rightColumn: "y" },
            { leftColumn: "p", rightColumn: "q" },
          ],
        },
      ],
      columns: [],
      filters: [],
    };
    expect(toStandardSql(q)).toBe('SELECT * FROM "a" INNER JOIN "b" ON "a"."x" = "b"."y" AND "a"."p" = "b"."q"');
  });

  it("renders count_distinct as COUNT(DISTINCT col) with the canonical alias", () => {
    const q: SqlBuilderQuery = {
      table: "t",
      columns: [{ name: "c", aggregation: "count_distinct" }],
      filters: [],
    };
    expect(toStandardSql(q)).toBe('SELECT COUNT(DISTINCT "c") AS "count_distinct_c" FROM "t"');
  });

  it("splits WHERE vs HAVING by isAggregate — HAVING emits the aggregate expression, never the alias", () => {
    const q: SqlBuilderQuery = {
      table: "t",
      columns: [{ name: "value", aggregation: "avg", alias: "avg_v" }],
      filters: [
        { column: "kind", operator: "=", value: "cpu" },
        { column: "value", operator: ">", value: 10, isAggregate: true, aggregation: "avg" },
      ],
      groupBy: ["kind"],
    };
    expect(toStandardSql(q)).toBe(
      'SELECT AVG("value") AS "avg_v" FROM "t" WHERE "kind" = \'cpu\' GROUP BY "kind" HAVING AVG("value") > 10',
    );
  });

  it("chains filters with AND/OR by each filter's `logical`", () => {
    const q: SqlBuilderQuery = {
      table: "t",
      columns: [],
      filters: [
        { column: "a", operator: "=", value: 1 },
        { column: "b", operator: ">", value: 2, logical: "OR" },
        { column: "c", operator: "<=", value: 3, logical: "AND" },
      ],
    };
    expect(toStandardSql(q)).toBe('SELECT * FROM "t" WHERE "a" = 1 OR "b" > 2 AND "c" <= 3');
  });

  it("renders LIKE / IS NULL / IS NOT NULL operators (no value for IS NULL / IS NOT NULL)", () => {
    const q: SqlBuilderQuery = {
      table: "t",
      columns: [],
      filters: [
        { column: "name", operator: "LIKE", value: "cpu%" },
        { column: "ts", operator: "IS NULL", logical: "AND" },
        { column: "ts", operator: "IS NOT NULL", logical: "OR" },
      ],
    };
    expect(toStandardSql(q)).toBe(
      'SELECT * FROM "t" WHERE "name" LIKE \'cpu%\' AND "ts" IS NULL OR "ts" IS NOT NULL',
    );
  });

  it("renders multi-column ORDER BY (qualified under joins, comma-separated)", () => {
    const q: SqlBuilderQuery = {
      table: "t",
      joins: [{ table: "u", type: "inner", on: [{ leftColumn: "id", rightColumn: "t_id" }] }],
      columns: [],
      filters: [],
      orderBy: [
        { column: "a", direction: "asc" },
        { column: "b", table: "u", direction: "desc" },
      ],
    };
    expect(toStandardSql(q)).toBe(
      'SELECT * FROM "t" INNER JOIN "u" ON "t"."id" = "u"."t_id" ORDER BY "t"."a" ASC, "u"."b" DESC',
    );
  });

  it("renders a qualified {table, column} groupBy entry under joins", () => {
    const q: SqlBuilderQuery = {
      table: "t",
      joins: [{ table: "u", type: "inner", on: [{ leftColumn: "id", rightColumn: "t_id" }] }],
      columns: [],
      filters: [],
      groupBy: [{ table: "u", column: "kind" }],
    };
    expect(toStandardSql(q)).toBe(
      'SELECT * FROM "t" INNER JOIN "u" ON "t"."id" = "u"."t_id" GROUP BY "u"."kind"',
    );
  });

  it("back-compat: a pre-slice query (no joins, single orderBy, string groupBy) emits byte-identically", () => {
    // Same shape a pre-slice persisted cell carries — no joins/having/aliases, single orderBy object.
    const legacy: SqlBuilderQuery = {
      table: "point_reading",
      columns: [{ name: "point_id" }, { name: "value", aggregation: "sum" }],
      filters: [{ column: "value", operator: ">", value: 3 }],
      groupBy: ["point_id"],
      orderBy: { column: "point_id", direction: "desc" },
      limit: 50,
    };
    expect(toStandardSql(legacy)).toBe(
      'SELECT "point_id", SUM("value") AS "sum_value" FROM "point_reading" WHERE "value" > 3 GROUP BY "point_id" ORDER BY "point_id" DESC LIMIT 50',
    );
  });

  it("back-compat: an empty/table-less query emits the empty string", () => {
    expect(toStandardSql(emptyQuery(""))).toBe("");
  });

  it("honours SqlColumn.order for SELECT ordering (stable; missing order sorts last)", () => {
    const q: SqlBuilderQuery = {
      table: "t",
      columns: [
        { name: "a", order: 3 },
        { name: "b" }, // missing order — sorts after ordered columns
        { name: "c", order: 1 },
      ],
      filters: [],
    };
    expect(toStandardSql(q)).toBe('SELECT "c", "a", "b" FROM "t"');
  });
});
