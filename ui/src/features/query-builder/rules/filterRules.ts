// The pure projection between the typed `SqlFilter[]` and react-querybuilder's
// `RuleGroupTypeIC` (the react-querybuilder slice). The model is the single source of truth;
// the <QueryBuilder> UI is a projection re-derived on every edit. One responsibility per file
// (FILE-LAYOUT): this is the only seam between the typed query filters and react-querybuilder's
// rule-group shape — no React here. Mirrors the discipline of `canvas/canvasModel.ts`.
//
// CRITICAL INVARIANT (rule 10 / the scope's governing invariant): we NEVER call react-querybuilder's
// `formatQuery()` — it would bypass our dialect seam (`emitSql` → `toStandardSql`/`toSurrealQL`) and
// has an injection surface. The projection goes ONE WAY for display (`SqlFilter[] → RuleGroupTypeIC`);
// on every edit (`onQueryChange`) we flatten BACK to `SqlFilter[]` via `fromRuleGroup`, write it into
// `SqlBuilderQuery.filters`, and let `emitSql(dialect, query)` render the SQL. The dialect emitters,
// the `ident()`/`renderValue()` injection guard, and the golden byte-identity all stay intact.
//
// independentCombinators: each rule carries its own combinator (`and`/`or`), which maps 1:1 to our
// flat `SqlFilter.logical` (`AND`/`OR`). The IC `rules` array is flat: `[rule, comb, rule, comb, rule]`
// (combinators at odd indices, rules at even). No model change, no emitter change.

import type { RuleGroupTypeIC, RuleType } from "react-querybuilder";

import type { Schema } from "@/lib/schema";
import type {
  SqlAggregation,
  SqlBuilderQuery,
  SqlFilter,
  SqlLogical,
  SqlOperator,
} from "@/lib/panel-kit/sql/query";

/** Our `SqlOperator` → react-querybuilder operator name. `like` is a CUSTOM operator (declared in
 *  `RQB_OPERATORS`); `null`/`notNull` are RQB built-ins with unary arity (no value editor). */
const SQL_TO_RQB_OP: Record<SqlOperator, string> = {
  "=": "=",
  "!=": "!=",
  ">": ">",
  ">=": ">=",
  "<": "<",
  "<=": "<=",
  LIKE: "like",
  "IS NULL": "null",
  "IS NOT NULL": "notNull",
};

/** The inverse: react-querybuilder operator name → our `SqlOperator`. */
const RQB_TO_SQL_OP: Record<string, SqlOperator> = Object.fromEntries(
  (Object.entries(SQL_TO_RQB_OP) as [SqlOperator, string][]).map(([sql, rqb]) => [rqb, sql]),
);

/** The operator list passed to `<QueryBuilder operators={…} />`. `like` is ours; the rest are RQB
 *  names. `null`/`notNull` carry `arity: "unary"` so RQB renders no value editor for them. */
export const RQB_OPERATORS: { name: string; label: string; arity?: "unary" }[] = [
  { name: "=", label: "=" },
  { name: "!=", label: "!=" },
  { name: ">", label: ">" },
  { name: ">=", label: ">=" },
  { name: "<", label: "<" },
  { name: "<=", label: "<=" },
  { name: "like", label: "like" },
  { name: "null", label: "is null", arity: "unary" },
  { name: "notNull", label: "is not null", arity: "unary" },
];

/** The combinator list for independentCombinators mode (RQB uses lowercase names). */
export const RQB_COMBINATORS = [
  { name: "and", label: "AND" },
  { name: "or", label: "OR" },
];

const VALUELESS_OPS = new Set(["null", "notNull"]);

/** True if `item` is a rule object (not a combinator string) in an IC `rules` array. */
function isRule(item: unknown): item is RuleType {
  return typeof item === "object" && item !== null && "field" in item && "operator" in item;
}

/** True if the RQB operator name is valueless (`null` / `notNull`). */
function isValuelessRqbOp(op: string): boolean {
  return VALUELESS_OPS.has(op);
}

/** Whether the query currently has joins (drives identifier qualification). */
function hasJoins(query: SqlBuilderQuery): boolean {
  return !!query.joins && query.joins.length > 0;
}

/** The RQB field `name` for a plain (WHERE) filter: `table.column` under joins, bare `column` when
 *  there are none (matches the emitter's bare-identifier back-compat — no behavioural change). */
function plainField(filter: SqlFilter, query: SqlBuilderQuery): string {
  const table = filter.table ?? query.table;
  return hasJoins(query) && table ? `${table}.${filter.column}` : filter.column;
}

/** Split an RQB plain-field name back into `{table?, column}`. Under joins the field is `table.column`;
 *  without joins it is bare `column`. The table is OMITTED when it equals the FROM table (matches the
 *  emitter's `filter.table ?? fromTable` default so round-trip is byte-stable). */
function splitPlainField(
  field: string,
  query: SqlBuilderQuery,
): { table?: string; column: string } {
  if (!hasJoins(query) || !field.includes(".")) return { column: field };
  const idx = field.indexOf(".");
  const table = field.slice(0, idx);
  const column = field.slice(idx + 1);
  return table === query.table ? { column } : { table, column };
}

/** Build one RQB rule from a `SqlFilter`. The `value` is OMITTED for valueless operators; HAVING
 *  rules carry `meta.aggregation` (preserved by RQB but never interpreted by core). */
function toRule(filter: SqlFilter, query: SqlBuilderQuery, isAggregate: boolean): RuleType {
  const op = SQL_TO_RQB_OP[filter.operator];
  const rule: RuleType = {
    field: plainField(filter, query),
    operator: op,
    value: isValuelessRqbOp(op) ? undefined : filter.value,
  };
  if (isAggregate) rule.meta = { aggregation: filter.aggregation ?? "count" };
  return rule;
}

/** Project the flat `SqlFilter[]` (WHERE or HAVING, selected by `isAggregate`) into an IC rule group.
 *  Each filter's `logical` becomes the combinator PRECEDING its rule (first rule has none). */
export function toRuleGroup(
  filters: SqlFilter[],
  query: SqlBuilderQuery,
  isAggregate: boolean,
): RuleGroupTypeIC {
  const selected = filters.filter((f) => !!f.isAggregate === isAggregate);
  const rules: unknown[] = [];
  selected.forEach((f, i) => {
    if (i > 0) rules.push((f.logical ?? "AND").toLowerCase());
    rules.push(toRule(f, query, isAggregate));
  });
  return { rules: rules as RuleGroupTypeIC["rules"] };
}

/** Flatten an IC rule group back into `SqlFilter[]`. Combinators (odd indices) set the NEXT rule's
 *  `logical`; the first rule defaults to `AND` (the emitter never emits a leading logical). HAVING
 *  rules pull `aggregation` from `rule.meta` (defaulting to `count`). */
export function fromRuleGroup(
  group: RuleGroupTypeIC,
  query: SqlBuilderQuery,
  isAggregate: boolean,
): SqlFilter[] {
  const out: SqlFilter[] = [];
  let pendingLogical: SqlLogical = "AND";
  const arr = group.rules as unknown[];
  for (let idx = 0; idx < arr.length; idx++) {
    const item = arr[idx];
    if (idx % 2 === 1) {
      // A combinator string — the logical for the NEXT rule.
      pendingLogical = typeof item === "string" && item.toLowerCase() === "or" ? "OR" : "AND";
      continue;
    }
    if (!isRule(item)) continue;
    const { table, column } = splitPlainField(item.field, query);
    const operator = RQB_TO_SQL_OP[item.operator] ?? "=";
    const filter: SqlFilter = { column, operator, logical: pendingLogical };
    if (table) filter.table = table;
    if (!isValuelessRqbOp(item.operator)) filter.value = item.value as string | number | boolean;
    if (isAggregate) {
      filter.isAggregate = true;
      filter.aggregation = (item.meta?.aggregation as SqlAggregation | undefined) ?? "count";
    }
    out.push(filter);
    pendingLogical = "AND";
  }
  return out;
}

/** Project the schema (FROM + joined tables' columns) into RQB `fields`. Qualified `table.column`
 *  under joins; bare `column` without (matches `plainField`/the emitter's back-compat). */
export function schemaToFields(
  schema: Schema,
  query: SqlBuilderQuery,
): { name: string; label: string }[] {
  const joined = hasJoins(query);
  const tables = [query.table, ...(query.joins ?? []).map((j) => j.table)];
  const out: { name: string; label: string }[] = [];
  for (const t of tables) {
    const entry = schema.tables.find((x) => x.name === t);
    if (!entry) continue;
    for (const c of entry.columns) {
      const name = joined ? `${t}.${c.name}` : c.name;
      out.push({ name, label: joined ? `${t}.${c.name}` : c.name });
    }
  }
  return out;
}

/** Find the rule at `path` in a FLAT top-level IC group (the only shape this slice produces).
 *  Returns `undefined` for nested paths or non-rule positions. */
export function ruleAtPath(group: RuleGroupTypeIC, path: number[]): RuleType | undefined {
  if (path.length !== 1) return undefined;
  const item = (group.rules as unknown[])[path[0]];
  return isRule(item) ? item : undefined;
}

/** Return a new IC group with the rule at `path` having its `meta` shallow-merged with `patch`.
 *  Used by the HAVING value editor to persist the aggregation pick without going through `value`.
 *  No-op for nested paths or non-rule positions (defensive). */
export function withRuleMeta(
  group: RuleGroupTypeIC,
  path: number[],
  patch: Record<string, unknown>,
): RuleGroupTypeIC {
  if (path.length !== 1) return group;
  const idx = path[0];
  const rules = [...(group.rules as unknown[])];
  const item = rules[idx];
  if (!isRule(item)) return group;
  rules[idx] = { ...item, meta: { ...(item.meta ?? {}), ...patch } };
  return { ...group, rules: rules as RuleGroupTypeIC["rules"] };
}
