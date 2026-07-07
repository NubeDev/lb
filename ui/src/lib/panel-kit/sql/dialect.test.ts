// Pure-logic unit tests for the dialect dispatch (query-builder-common scope). The seam is small
// enough to pin exhaustively: for every `SqlDialect`, `emitSql(d, q)` returns the matching
// emitter's output byte-for-byte, and the two emitters DIFFER on the same query (the point of
// having two). The per-emitter goldens live in `toSurrealQL.test.ts` and `toStandardSql.test.ts`.

import { describe, expect, it } from "vitest";

import { emptyQuery, type SqlBuilderQuery } from "./query";
import { emitSql } from "./dialect";
import { toSurrealQL } from "./toSurrealQL";
import { toStandardSql } from "./toStandardSql";

describe("emitSql dispatch", () => {
  const q: SqlBuilderQuery = {
    ...emptyQuery("series"),
    columns: [{ name: "payload", aggregation: "avg" }],
    filters: [{ column: "series", operator: "=", value: "cpu" }],
    limit: 10,
  };

  it("routes `surreal` to toSurrealQL byte-for-byte", () => {
    expect(emitSql("surreal", q)).toBe(toSurrealQL(q));
  });

  it("routes `standard` to toStandardSql byte-for-byte", () => {
    expect(emitSql("standard", q)).toBe(toStandardSql(q));
  });

  it("the two dialects DIFFER on the same query (the reason for two emitters)", () => {
    // SurrealQL: math::avg(payload), bare identifier. Standard: AVG("payload"), double-quoted.
    expect(emitSql("surreal", q)).not.toBe(emitSql("standard", q));
  });

  it("both dialects return empty for a table-less builder query", () => {
    expect(emitSql("surreal", emptyQuery(""))).toBe("");
    expect(emitSql("standard", emptyQuery(""))).toBe("");
  });

  // visual-canvas-builder slice: joins are standard-only; surreal drops them defensively.

  it("a query WITH joins: standard emits the JOIN clause, surreal drops it (no JOIN in output)", () => {
    const qWithJoins: SqlBuilderQuery = {
      table: "site",
      joins: [{ table: "point_reading", type: "inner", on: [{ leftColumn: "id", rightColumn: "site_id" }] }],
      columns: [{ name: "name" }, { name: "value" }],
      filters: [],
    };
    const std = emitSql("standard", qWithJoins);
    const sur = emitSql("surreal", qWithJoins);
    expect(std).toContain('INNER JOIN "point_reading"');
    expect(sur).not.toMatch(/JOIN/i);
    // The surreal emit still renders the single-table SELECT (the FROM is preserved).
    expect(sur).toBe("SELECT name, value FROM site");
  });

  it("a query with HAVING: both dialects emit a HAVING clause (surreal: math::agg, standard: AVG())", () => {
    const qWithHaving: SqlBuilderQuery = {
      table: "t",
      columns: [{ name: "value", aggregation: "avg", alias: "avg_v" }],
      filters: [
        { column: "kind", operator: "=", value: "cpu" },
        { column: "value", operator: ">", value: 10, isAggregate: true, aggregation: "avg" },
      ],
      groupBy: ["kind"],
    };
    expect(emitSql("standard", qWithHaving)).toContain('HAVING AVG("value") > 10');
    expect(emitSql("surreal", qWithHaving)).toContain("HAVING math::avg(value) > 10");
  });
});
