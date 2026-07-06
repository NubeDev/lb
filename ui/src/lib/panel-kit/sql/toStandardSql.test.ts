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
});
