// Round-trip contract for the SQL→model parsers (query-builder slice-1 follow-up: Code→Builder
// sync): for every builder query the emitter goldens cover, `parse(emitSql(dialect, q))` is
// SEMANTICALLY equal to `q` — pinned property-style as `emit(parse(emit(q))) === emit(q)` (the
// emitter is the canonical form, so equal re-emission ⇔ semantic equality). Plus the negative
// contract: SQL the model cannot express (subquery / CTE / window fn / DISTINCT select /
// multi-statement / PRQL-ish garbage) parses to `null`, never a wrong model — and injection-shaped
// input never throws. Structural spot-checks pin the headline Code→Builder case (the live-repro
// JOIN) so a canonicalization bug can't hide behind the property.

import { describe, expect, it } from "vitest";

import { emptyQuery, type SqlBuilderQuery } from "./query";
import { toStandardSql } from "./toStandardSql";
import { toSurrealQL } from "./toSurrealQL";
import { parseStandardSql } from "./fromStandardSql";
import { parseSurrealQL } from "./fromSurrealQL";
import { parseSql, salvageFromTable } from "./parseSql";

// ── The emitter-golden fixtures (mirrors of every query in toStandardSql.test.ts) ──────────────

const STANDARD_FIXTURES: Record<string, SqlBuilderQuery> = {
  bareSelectStar: emptyQuery("point_reading"),
  explicitColumns: {
    ...emptyQuery("point_reading"),
    columns: [{ name: "point_id" }, { name: "value" }],
  },
  countStarAvg: {
    ...emptyQuery("point_reading"),
    columns: [
      { name: "*", aggregation: "count" },
      { name: "value", aggregation: "avg" },
    ],
  },
  sumMinMax: {
    ...emptyQuery("reading"),
    columns: [
      { name: "value", aggregation: "sum" },
      { name: "value", aggregation: "min" },
      { name: "value", aggregation: "max" },
    ],
  },
  countColumn: {
    ...emptyQuery("reading"),
    columns: [{ name: "value", aggregation: "count" }],
  },
  whereStringAndNumber: {
    ...emptyQuery("point_reading"),
    filters: [
      { column: "point_id", operator: "=", value: "p1" },
      { column: "value", operator: ">", value: 3 },
    ],
  },
  escapedQuoteValue: {
    ...emptyQuery("site"),
    filters: [{ column: "name", operator: "=", value: "o'brien" }],
  },
  escapedIdentifiers: {
    ...emptyQuery('weird " table'),
    columns: [{ name: 'col " name' }],
  },
  groupOrderLimit: {
    ...emptyQuery("point_reading"),
    columns: [{ name: "point_id" }, { name: "value", aggregation: "sum" }],
    groupBy: ["point_id"],
    orderBy: { column: "point_id", direction: "desc" },
    limit: 50,
  },
  singleInnerJoin: {
    table: "site",
    joins: [{ table: "point_reading", type: "inner", on: [{ leftColumn: "id", rightColumn: "site_id" }] }],
    columns: [{ name: "name", table: "site" }, { name: "value", table: "point_reading" }],
    filters: [],
  },
  leftJoin: joinOfType("left"),
  rightJoin: joinOfType("right"),
  fullJoin: joinOfType("full"),
  crossJoin: joinOfType("cross"),
  pendingJoinDropped: {
    table: "site",
    joins: [{ table: "site_tag", type: "inner", on: [] }],
    columns: [{ name: "name", table: "site" }, { name: "tag", table: "site_tag" }],
    filters: [
      { column: "name", table: "site", operator: "=", value: "hq" },
      { column: "tag", table: "site_tag", operator: "=", value: "x" },
    ],
    groupBy: [{ table: "site_tag", column: "tag" }],
    orderBy: [{ column: "tag", table: "site_tag", direction: "asc" }],
  },
  pendingBesideWired: {
    table: "site",
    joins: [
      { table: "point_reading", type: "inner", on: [{ leftColumn: "id", rightColumn: "site_id" }] },
      { table: "site_tag", type: "inner", on: [] },
    ],
    columns: [{ name: "value", table: "point_reading" }, { name: "tag", table: "site_tag" }],
    filters: [],
  },
  allColumnsPending: {
    table: "site",
    joins: [{ table: "site_tag", type: "inner", on: [] }],
    columns: [{ name: "tag", table: "site_tag" }],
    filters: [],
  },
  compositeOn: {
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
  },
  countDistinct: {
    table: "t",
    columns: [{ name: "c", aggregation: "count_distinct" }],
    filters: [],
  },
  whereVsHaving: {
    table: "t",
    columns: [{ name: "value", aggregation: "avg", alias: "avg_v" }],
    filters: [
      { column: "kind", operator: "=", value: "cpu" },
      { column: "value", operator: ">", value: 10, isAggregate: true, aggregation: "avg" },
    ],
    groupBy: ["kind"],
  },
  andOrChain: {
    table: "t",
    columns: [],
    filters: [
      { column: "a", operator: "=", value: 1 },
      { column: "b", operator: ">", value: 2, logical: "OR" },
      { column: "c", operator: "<=", value: 3, logical: "AND" },
    ],
  },
  likeAndNulls: {
    table: "t",
    columns: [],
    filters: [
      { column: "name", operator: "LIKE", value: "cpu%" },
      { column: "ts", operator: "IS NULL", logical: "AND" },
      { column: "ts", operator: "IS NOT NULL", logical: "OR" },
    ],
  },
  multiOrderBy: {
    table: "t",
    joins: [{ table: "u", type: "inner", on: [{ leftColumn: "id", rightColumn: "t_id" }] }],
    columns: [],
    filters: [],
    orderBy: [
      { column: "a", direction: "asc" },
      { column: "b", table: "u", direction: "desc" },
    ],
  },
  qualifiedGroupBy: {
    table: "t",
    joins: [{ table: "u", type: "inner", on: [{ leftColumn: "id", rightColumn: "t_id" }] }],
    columns: [],
    filters: [],
    groupBy: [{ table: "u", column: "kind" }],
  },
  legacyPreSlice: {
    table: "point_reading",
    columns: [{ name: "point_id" }, { name: "value", aggregation: "sum" }],
    filters: [{ column: "value", operator: ">", value: 3 }],
    groupBy: ["point_id"],
    orderBy: { column: "point_id", direction: "desc" },
    limit: 50,
  },
  aliasOrderBy: {
    // ORDER BY on a SELECT alias (an ANSI output-name sort) — must emit the bare alias, never
    // `"table"."alias"` (the live bug: the engine rejects the phantom qualified column).
    table: "site",
    joins: [{ table: "point_reading", type: "inner", on: [{ leftColumn: "id", rightColumn: "site_id" }] }],
    columns: [
      { name: "name", table: "site", alias: "site_name" },
      { name: "value", table: "point_reading", aggregation: "avg", alias: "avg_energy" },
    ],
    filters: [],
    groupBy: [{ table: "site", column: "name" }],
    orderBy: [{ column: "avg_energy", direction: "desc" }],
  },
  defaultAggAliasOrderBy: {
    // Same, via the aggregate's DEFAULT alias (`sum_value`) — no explicit alias set.
    table: "t",
    columns: [{ name: "kind" }, { name: "value", aggregation: "sum" }],
    filters: [],
    groupBy: ["kind"],
    orderBy: [{ column: "sum_value", direction: "desc" }],
  },
  columnOrder: {
    table: "t",
    columns: [
      { name: "a", order: 3 },
      { name: "b" },
      { name: "c", order: 1 },
    ],
    filters: [],
  },
};

function joinOfType(type: "left" | "right" | "full" | "cross"): SqlBuilderQuery {
  return {
    table: "a",
    joins: [{ table: "b", type, on: [{ leftColumn: "x", rightColumn: "y" }] }],
    columns: [],
    filters: [],
  };
}

// The surreal subset (mirrors of every query in toSurrealQL.test.ts; the joins fixture is included —
// the emitter drops joins, so the parse of its output is the joinless query, still re-emitting equal).
const SURREAL_FIXTURES: Record<string, SqlBuilderQuery> = {
  bareSelectStar: emptyQuery("series"),
  explicitColumns: { ...emptyQuery("series"), columns: [{ name: "seq" }, { name: "payload" }] },
  countAvg: {
    ...emptyQuery("series"),
    columns: [
      { name: "*", aggregation: "count" },
      { name: "payload", aggregation: "avg" },
    ],
  },
  whereStringAndNumber: {
    ...emptyQuery("series"),
    filters: [
      { column: "series", operator: "=", value: "cpu" },
      { column: "seq", operator: ">", value: 3 },
    ],
  },
  escapedQuoteValue: {
    ...emptyQuery("series"),
    filters: [{ column: "name", operator: "=", value: "o'brien" }],
  },
  groupOrderLimit: {
    ...emptyQuery("series"),
    columns: [{ name: "series" }, { name: "payload", aggregation: "sum" }],
    groupBy: ["series"],
    orderBy: { column: "series", direction: "desc" },
    limit: 50,
  },
  having: {
    table: "series",
    columns: [{ name: "payload", aggregation: "avg", alias: "avg_p" }],
    filters: [
      { column: "series", operator: "=", value: "cpu" },
      { column: "payload", operator: ">", value: 10, isAggregate: true, aggregation: "avg" },
    ],
    groupBy: ["series"],
  },
  andOr: {
    table: "t",
    columns: [],
    filters: [
      { column: "a", operator: "=", value: 1 },
      { column: "b", operator: ">", value: 2, logical: "OR" },
    ],
  },
  likeAndNull: {
    table: "t",
    columns: [],
    filters: [
      { column: "name", operator: "LIKE", value: "cpu%" },
      { column: "ts", operator: "IS NULL", logical: "AND" },
    ],
  },
  multiOrderBy: {
    table: "t",
    columns: [],
    filters: [],
    orderBy: [
      { column: "a", direction: "asc" },
      { column: "b", direction: "desc" },
    ],
  },
  countDistinct: {
    table: "t",
    columns: [{ name: "c", aggregation: "count_distinct" }],
    filters: [],
  },
  joinsDropped: {
    table: "t",
    joins: [{ table: "u", type: "inner", on: [{ leftColumn: "id", rightColumn: "t_id" }] }],
    columns: [],
    filters: [],
  },
  builderRoundtrip: {
    table: "series",
    columns: [{ name: "seq" }, { name: "payload", aggregation: "max" }],
    filters: [{ column: "seq", operator: ">=", value: 2 }],
    groupBy: ["seq"],
    orderBy: { column: "seq", direction: "asc" },
    limit: 100,
  },
};

// ── The property: parse(emit(q)) re-emits the identical string, for every golden ────────────────

describe("standard round-trip: toStandardSql → parseStandardSql → toStandardSql is identity", () => {
  for (const [name, q] of Object.entries(STANDARD_FIXTURES)) {
    it(name, () => {
      const sql = toStandardSql(q);
      const parsed = parseStandardSql(sql);
      expect(parsed, `parse failed for: ${sql}`).not.toBeNull();
      expect(toStandardSql(parsed!)).toBe(sql);
    });
  }
});

describe("surreal round-trip: toSurrealQL → parseSurrealQL → toSurrealQL is identity", () => {
  for (const [name, q] of Object.entries(SURREAL_FIXTURES)) {
    it(name, () => {
      const sql = toSurrealQL(q);
      const parsed = parseSurrealQL(sql);
      expect(parsed, `parse failed for: ${sql}`).not.toBeNull();
      expect(toSurrealQL(parsed!)).toBe(sql);
    });
  }
});

// ── Structural spot-checks (the live-repro JOIN + hand-written variants) ────────────────────────

describe("parseStandardSql structure", () => {
  it("parses the live-repro hand-typed JOIN into the typed model", () => {
    const q = parseStandardSql(
      'SELECT "name" FROM "site" INNER JOIN "site_tag" ON "site"."id" = "site_tag"."site_id"',
    );
    expect(q).toEqual({
      table: "site",
      joins: [{ table: "site_tag", type: "inner", on: [{ leftColumn: "id", rightColumn: "site_id" }] }],
      // `"name"` is written unqualified — the parser keeps it that way; the emitter qualifies an
      // unqualified column with the FROM table under joins, so the round-trip stays semantic.
      columns: [{ name: "name" }],
      filters: [],
      groupBy: [],
    });
  });

  it("accepts hand-written variants: bare identifiers, lowercase keywords, <>, bare JOIN, trailing ;", () => {
    const q = parseStandardSql(
      "select name, count(*) as n from site join site_tag on site_tag.site_id = site.id where kind <> 'x' group by name order by n desc limit 10;",
    );
    expect(q).toMatchObject({
      table: "site",
      joins: [{ table: "site_tag", type: "inner", on: [{ leftColumn: "id", rightColumn: "site_id" }] }],
      filters: [{ column: "kind", operator: "!=", value: "x" }],
      groupBy: ["name"],
      limit: 10,
    });
  });

  it("orients a flipped ON equality (joined table on the left) into leftColumn/rightColumn", () => {
    const q = parseStandardSql('SELECT * FROM "a" INNER JOIN "b" ON "b"."y" = "a"."x"');
    expect(q?.joins).toEqual([{ table: "b", type: "inner", on: [{ leftColumn: "x", rightColumn: "y" }] }]);
  });

  it("resolves table aliases (FROM site s, JOIN meter m, s.name, GROUP BY s.id) to real tables", () => {
    // The live bug report: a hand-written aliased multi-join query fell to the confirm path and the
    // builder came up empty. It IS expressible — aliases resolve to their tables in the model.
    const q = parseStandardSql(`SELECT
        s.name AS site_name,
        AVG(r.value) AS avg_energy
      FROM site s
      JOIN meter m ON m.site_id = s.id
      JOIN point p ON p.meter_id = m.id
      JOIN point_reading r ON r.point_id = p.id
      GROUP BY s.id, s.name
      ORDER BY avg_energy DESC;`);
    expect(q).toEqual({
      table: "site",
      joins: [
        { table: "meter", type: "inner", on: [{ leftColumn: "id", rightColumn: "site_id" }] },
        { table: "point", type: "inner", on: [{ leftTable: "meter", leftColumn: "id", rightColumn: "meter_id" }] },
        { table: "point_reading", type: "inner", on: [{ leftTable: "point", leftColumn: "id", rightColumn: "point_id" }] },
      ],
      columns: [
        { name: "name", table: "site", alias: "site_name" },
        { name: "value", table: "point_reading", aggregation: "avg", alias: "avg_energy" },
      ],
      filters: [],
      groupBy: [
        { table: "site", column: "id" },
        { table: "site", column: "name" },
      ],
      orderBy: [{ column: "avg_energy", direction: "desc" }],
    });
    // And it re-emits as a runnable, alias-free canonical query — the alias sort stays BARE
    // (`ORDER BY "avg_energy"`, an output-name reference, never `"site"."avg_energy"`).
    expect(toStandardSql(q!)).toBe(
      'SELECT "site"."name" AS "site_name", AVG("point_reading"."value") AS "avg_energy" FROM "site"' +
        ' INNER JOIN "meter" ON "site"."id" = "meter"."site_id"' +
        ' INNER JOIN "point" ON "meter"."id" = "point"."meter_id"' +
        ' INNER JOIN "point_reading" ON "point"."id" = "point_reading"."point_id"' +
        ' GROUP BY "site"."id", "site"."name"' +
        ' ORDER BY "avg_energy" DESC',
    );
  });

  it("rejects a self-join (same table under two aliases — not expressible in the model)", () => {
    expect(parseStandardSql("SELECT * FROM emp a JOIN emp b ON b.manager_id = a.id")).toBeNull();
  });

  it("records leftTable when a later join keys off a previously-joined table", () => {
    const q = parseStandardSql(
      'SELECT * FROM "a" INNER JOIN "b" ON "a"."x" = "b"."y" LEFT JOIN "c" ON "b"."z" = "c"."w"',
    );
    expect(q?.joins?.[1]).toEqual({
      table: "c",
      type: "left",
      on: [{ leftTable: "b", leftColumn: "z", rightColumn: "w" }],
    });
  });
});

// ── Not-expressible SQL parses to null (the confirm path), never a wrong model ──────────────────

const NOT_EXPRESSIBLE = [
  ["subquery in FROM", "SELECT * FROM (SELECT * FROM t) x"],
  ["subquery in WHERE", "SELECT * FROM t WHERE id IN (SELECT id FROM u)"],
  ["CTE", "WITH x AS (SELECT 1) SELECT * FROM x"],
  ["window function", 'SELECT ROW_NUMBER() OVER (ORDER BY "a") FROM t'],
  ["multi-statement", "SELECT * FROM t; SELECT * FROM u"],
  ["write statement", "DELETE FROM t"],
  ["SELECT DISTINCT", "SELECT DISTINCT a FROM t"],
  ["parenthesized boolean group", "SELECT * FROM t WHERE (a = 1 OR b = 2) AND c = 3"],
  ["column-to-column WHERE", "SELECT * FROM t WHERE a = b"],
  ["arithmetic expression", "SELECT a + 1 FROM t"],
  ["empty string", ""],
  ["not SQL at all", "from site | select name"],
] as const;

describe("not-expressible SQL returns null (both dialects)", () => {
  for (const [name, sql] of NOT_EXPRESSIBLE) {
    it(name, () => {
      expect(parseSql("standard", sql)).toBeNull();
      expect(parseSql("surreal", sql)).toBeNull();
    });
  }

  it("surreal: ANSI JOIN is not expressible (surreal has no joins)", () => {
    expect(parseSurrealQL("SELECT * FROM a INNER JOIN b ON a.x = b.y")).toBeNull();
  });
});

// ── Injection-shaped / hostile input never throws ───────────────────────────────────────────────

describe("hostile input never panics", () => {
  const HOSTILE = [
    "SELECT * FROM t WHERE a = 'unterminated",
    'SELECT "unterminated FROM t',
    "SELECT * FROM t; DROP TABLE t; --",
    "SELECT * FROM t WHERE a = ''' OR '1'='1'",
    "'; DROP TABLE users; --",
    "SELECT   FROM  ",
    "SELECT * FROM t WHERE a = $1",
    "`backticks` everywhere `",
  ];
  for (const sql of HOSTILE) {
    it(JSON.stringify(sql.slice(0, 40)), () => {
      expect(() => parseSql("standard", sql)).not.toThrow();
      expect(() => parseSql("surreal", sql)).not.toThrow();
      expect(() => salvageFromTable(sql)).not.toThrow();
    });
  }

  it("a quoted-string value with embedded quotes round-trips through parse → emit safely", () => {
    const q = parseStandardSql(`SELECT * FROM "t" WHERE "a" = 'o''brien'`);
    expect(q?.filters).toEqual([{ column: "a", operator: "=", value: "o'brien" }]);
    expect(toStandardSql(q!)).toBe(`SELECT * FROM "t" WHERE "a" = 'o''brien'`);
  });
});

// ── The salvage helper (the confirm path's FROM-table recovery) ─────────────────────────────────

describe("salvageFromTable", () => {
  it("recovers the FROM table from unparseable-but-lexable SQL", () => {
    expect(salvageFromTable('SELECT a + 1 FROM "site" WHERE x')).toBe("site");
    expect(salvageFromTable("SELECT DISTINCT a FROM site")).toBe("site");
  });
  it("returns empty when there is no simple FROM table", () => {
    expect(salvageFromTable("SELECT * FROM (SELECT 1) x")).toBe("");
    expect(salvageFromTable("not sql")).toBe("");
  });
});
