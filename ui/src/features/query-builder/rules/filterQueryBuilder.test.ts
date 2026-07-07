// Pure unit tests for the react-querybuilder projection (the react-querybuilder slice). The typed
// `SqlFilter[]` is the single source of truth; `toRuleGroup` projects it into RQB's
// `RuleGroupTypeIC` for display, and `fromRuleGroup` flattens an edited group back into `SqlFilter[]`.
// The round-trip preserves `logical` + `operator` + `value` + `isAggregate`; the emitters never
// change (the golden bytes stay byte-identical). No React, no jsdom.

import { describe, expect, it } from "vitest";

import type { SqlBuilderQuery, SqlFilter, SqlOperator } from "@/lib/panel-kit/sql/query";
import type { Schema } from "@/lib/schema";
import {
  fromRuleGroup,
  RQB_COMBINATORS,
  RQB_OPERATORS,
  ruleAtPath,
  schemaToFields,
  toRuleGroup,
  withRuleMeta,
} from "./filterQueryBuilder";

const schema: Schema = {
  tables: [
    { name: "site", columns: [{ name: "id", type: "string" }, { name: "name", type: "string" }] },
    { name: "reading", columns: [{ name: "site_id", type: "string" }, { name: "value", type: "float" }] },
  ],
};

const baseQuery: SqlBuilderQuery = { table: "site", columns: [], filters: [] };

/** All 9 SqlOperators — round-trip coverage. */
const ALL_OPERATORS: SqlOperator[] = ["=", "!=", ">", ">=", "<", "<=", "LIKE", "IS NULL", "IS NOT NULL"];

describe("filterQueryBuilder.toRuleGroup", () => {
  it("projects a flat WHERE filter list into an IC group with combinators between rules", () => {
    const filters: SqlFilter[] = [
      { column: "name", operator: "=", value: "x", logical: "AND" },
      { column: "id", operator: ">", value: 5, logical: "OR" },
    ];
    const group = toRuleGroup(filters, baseQuery, false);
    // [rule, "or", rule] — first rule has no preceding combinator; second carries OR.
    expect(group.rules).toHaveLength(3);
    expect((group.rules as unknown[])[1]).toBe("or");
    const r0 = (group.rules as unknown[])[0] as { field: string; operator: string; value: unknown };
    const r2 = (group.rules as unknown[])[2] as { field: string; operator: string; value: unknown };
    expect(r0.field).toBe("name");
    expect(r0.operator).toBe("=");
    expect(r0.value).toBe("x");
    expect(r2.field).toBe("id");
    expect(r2.operator).toBe(">");
    expect(r2.value).toBe(5);
  });

  it("returns an empty IC group when no filters match the aggregate flag", () => {
    const filters: SqlFilter[] = [{ column: "name", operator: "=", value: "x" }];
    expect(toRuleGroup(filters, baseQuery, true).rules).toHaveLength(0);
    expect(toRuleGroup([], baseQuery, false).rules).toHaveLength(0);
  });

  it("OMITS the value on valueless operators (null / notNull)", () => {
    const filters: SqlFilter[] = [
      { column: "name", operator: "IS NULL" },
      { column: "id", operator: "IS NOT NULL" },
    ];
    const group = toRuleGroup(filters, baseQuery, false);
    const r0 = (group.rules as unknown[])[0] as { value?: unknown; operator: string };
    const r2 = (group.rules as unknown[])[2] as { value?: unknown; operator: string };
    expect(r0.operator).toBe("null");
    expect(r0.value).toBeUndefined();
    expect(r2.operator).toBe("notNull");
    expect(r2.value).toBeUndefined();
  });

  it("puts HAVING (isAggregate) rules into the HAVING group, plain rules into WHERE", () => {
    const filters: SqlFilter[] = [
      { column: "name", operator: "=", value: "x" },
      { column: "value", operator: ">", value: 10, isAggregate: true, aggregation: "avg" },
    ];
    const where = toRuleGroup(filters, baseQuery, false);
    const having = toRuleGroup(filters, baseQuery, true);
    expect(where.rules).toHaveLength(1);
    expect(having.rules).toHaveLength(1);
    const havingRule = (having.rules as unknown[])[0] as { meta?: { aggregation?: string } };
    expect(havingRule.meta?.aggregation).toBe("avg");
  });
});

describe("filterQueryBuilder.fromRuleGroup (round-trip)", () => {
  it("round-trips a mixed AND/OR WHERE filter list (logical + operator + value preserved)", () => {
    const original: SqlFilter[] = [
      { column: "name", operator: "=", value: "x", logical: "AND" },
      { column: "id", operator: ">", value: 5, logical: "OR" },
      { column: "value", operator: "<=", value: 100, logical: "AND" },
    ];
    const round = fromRuleGroup(toRuleGroup(original, baseQuery, false), baseQuery, false);
    expect(round).toEqual(original);
  });

  it("round-trips EVERY SqlOperator (the full operator-mapping table)", () => {
    const original: SqlFilter[] = ALL_OPERATORS.map((op, i) => {
      const valueless = op === "IS NULL" || op === "IS NOT NULL";
      return {
        column: "name",
        operator: op,
        ...(valueless ? {} : { value: i % 2 === 0 ? "v" : i }),
        logical: i === 0 ? "AND" : i % 2 === 0 ? "AND" : "OR",
      } as SqlFilter;
    });
    const round = fromRuleGroup(toRuleGroup(original, baseQuery, false), baseQuery, false);
    expect(round).toEqual(original);
  });

  it("round-trips HAVING filters (isAggregate + aggregation preserved)", () => {
    const original: SqlFilter[] = [
      { column: "value", operator: ">", value: 10, isAggregate: true, aggregation: "avg", logical: "AND" },
      { column: "id", operator: "=", value: 3, isAggregate: true, aggregation: "count", logical: "OR" },
    ];
    const round = fromRuleGroup(toRuleGroup(original, baseQuery, true), baseQuery, true);
    expect(round).toEqual(original);
  });

  it("defaults a HAVING rule with no meta.aggregation to count (defensive)", () => {
    // A rule the user just added via RQB's "+ Rule" — no meta on it yet.
    const group = { rules: [{ field: "value", operator: ">", value: 0 }] } as never;
    const round = fromRuleGroup(group, baseQuery, true);
    expect(round).toEqual([
      { column: "value", operator: ">", value: 0, logical: "AND", isAggregate: true, aggregation: "count" },
    ]);
  });

  it("qualifies fields under joins and round-trips them (table preserved for non-FROM tables)", () => {
    const joined: SqlBuilderQuery = {
      table: "site",
      joins: [{ table: "reading", type: "inner", on: [{ leftColumn: "id", rightColumn: "site_id" }] }],
      columns: [],
      filters: [],
    };
    const original: SqlFilter[] = [
      { column: "name", operator: "=", value: "x", logical: "AND" },
      { column: "value", table: "reading", operator: ">", value: 1, logical: "OR" },
    ];
    const round = fromRuleGroup(toRuleGroup(original, joined, false), joined, false);
    expect(round).toEqual(original);
  });

  it("the first rule's logical defaults to AND after round-trip", () => {
    // The first filter carries no preceding combinator in IC mode; fromRuleGroup defaults it to AND.
    const original: SqlFilter[] = [{ column: "name", operator: "=", value: "x", logical: "AND" }];
    const round = fromRuleGroup(toRuleGroup(original, baseQuery, false), baseQuery, false);
    expect(round[0].logical).toBe("AND");
  });
});

describe("filterQueryBuilder.operator map", () => {
  it("RQB_OPERATORS declares every operator the map can produce (no orphan names)", () => {
    const declared = new Set(RQB_OPERATORS.map((o) => o.name));
    for (const op of ALL_OPERATORS) {
      // Every SqlOperator must survive the round-trip (covered above), which requires its RQB
      // counterpart to be declared so <QueryBuilder> recognises it.
      const rqbName = op === "=" ? "="
        : op === "!=" ? "!="
        : op === ">" ? ">"
        : op === ">=" ? ">="
        : op === "<" ? "<"
        : op === "<=" ? "<="
        : op === "LIKE" ? "like"
        : op === "IS NULL" ? "null"
        : "notNull";
      expect(declared.has(rqbName)).toBe(true);
    }
  });

  it("marks null/notNull as arity unary (so RQB renders no value editor)", () => {
    const nullOp = RQB_OPERATORS.find((o) => o.name === "null");
    const notNullOp = RQB_OPERATORS.find((o) => o.name === "notNull");
    expect(nullOp?.arity).toBe("unary");
    expect(notNullOp?.arity).toBe("unary");
  });

  it("declares the and/or combinators IC mode uses", () => {
    expect(RQB_COMBINATORS.map((c) => c.name)).toEqual(["and", "or"]);
  });
});

describe("filterQueryBuilder.schemaToFields", () => {
  it("produces bare column names when there are no joins (back-compat with the emitter)", () => {
    const fields = schemaToFields(schema, baseQuery);
    expect(fields.map((f) => f.name)).toEqual(["id", "name"]);
  });

  it("qualifies fields as table.column under joins (FROM + joined tables)", () => {
    const joined: SqlBuilderQuery = {
      table: "site",
      joins: [{ table: "reading", type: "inner", on: [{ leftColumn: "id", rightColumn: "site_id" }] }],
      columns: [],
      filters: [],
    };
    const fields = schemaToFields(schema, joined);
    expect(fields.map((f) => f.name)).toEqual([
      "site.id",
      "site.name",
      "reading.site_id",
      "reading.value",
    ]);
  });

  it("returns an empty list when the FROM table is not in the schema", () => {
    expect(schemaToFields(schema, { table: "missing", columns: [], filters: [] })).toEqual([]);
  });
});

describe("filterQueryBuilder.withRuleMeta / ruleAtPath", () => {
  it("reads back a rule by its flat top-level path", () => {
    const group = toRuleGroup(
      [{ column: "value", operator: ">", value: 1, isAggregate: true, aggregation: "sum" }],
      baseQuery,
      true,
    );
    expect(ruleAtPath(group, [0])?.meta?.aggregation).toBe("sum");
    expect(ruleAtPath(group, [1])).toBeUndefined(); // combinator position (or absent)
  });

  it("patches a rule's meta without touching the rest of the group (the aggregation pick)", () => {
    const group = toRuleGroup(
      [
        { column: "value", operator: ">", value: 1, isAggregate: true, aggregation: "avg" },
        { column: "id", operator: "=", value: 2, isAggregate: true, aggregation: "count", logical: "OR" },
      ],
      baseQuery,
      true,
    );
    const patched = withRuleMeta(group, [0], { aggregation: "max" });
    expect(ruleAtPath(patched, [0])?.meta?.aggregation).toBe("max");
    // The second rule is untouched.
    expect(ruleAtPath(patched, [2])?.meta?.aggregation).toBe("count");
  });

  it("is a no-op for a nested path (this slice never produces nested groups)", () => {
    const group = toRuleGroup(
      [{ column: "value", operator: ">", value: 1, isAggregate: true, aggregation: "sum" }],
      baseQuery,
      true,
    );
    expect(withRuleMeta(group, [0, 1], { aggregation: "max" })).toBe(group);
  });
});
