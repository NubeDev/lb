// The SurrealQL → `SqlBuilderQuery` parser (query-builder slice-1 follow-up: Code→Builder sync) —
// the inverse of `toSurrealQL.ts`, for the subset that emitter produces: bare identifiers, `count()`
// / `count(col)` / `count(DISTINCT col)` / `math::sum|avg|min|max(col)` aggregates, bare aliases,
// WHERE/HAVING with AND/OR + LIKE + IS [NOT] NULL, GROUP BY (bare or `table.column`), multi ORDER BY,
// LIMIT. No joins — SurrealQL has none in the model (visual-canvas-builder scope: standard-only), so
// any JOIN-ish syntax simply fails the parse and returns `null` ("not expressible"). Same
// model-as-truth rule as `fromStandardSql.ts`: the parser produces a model; the emitter stays the
// only SQL writer.

import {
  type SqlAggregation,
  type SqlBuilderQuery,
  type SqlColumn,
  type SqlFilter,
  type SqlGroupByEntry,
  type SqlLogical,
  type SqlOperator,
  type SqlOrderBy,
} from "./query";
import { SqlParseError, TokenCursor, tokenize } from "./sqlTokens";

/** Keywords that can never be a bare identifier in the emitter subset. */
const RESERVED = new Set([
  "SELECT", "FROM", "WHERE", "GROUP", "HAVING", "ORDER", "LIMIT", "BY", "AS", "AND", "OR",
  "IS", "NULL", "NOT", "LIKE", "IN", "ASC", "DESC", "DISTINCT", "TRUE", "FALSE",
]);

const MATH_AGGS: Record<string, SqlAggregation> = { sum: "sum", avg: "avg", min: "min", max: "max" };

/** Parse a SurrealQL SELECT (the `toSurrealQL` subset) into the typed builder query, or `null` when
 *  not expressible. Never throws. */
export function parseSurrealQL(sql: string): SqlBuilderQuery | null {
  try {
    return parse(new TokenCursor(tokenize(sql)));
  } catch {
    return null;
  }
}

function parse(c: TokenCursor): SqlBuilderQuery {
  c.expectWord("SELECT");
  const columns = parseSelectList(c);
  c.expectWord("FROM");
  const table = parseIdent(c);

  const filters: SqlFilter[] = [];
  if (c.eatWord("WHERE")) filters.push(...parseFilterChain(c, false));

  let groupBy: SqlGroupByEntry[] | undefined;
  if (c.eatWord("GROUP")) {
    c.expectWord("BY");
    groupBy = [];
    do {
      const first = parseIdent(c);
      groupBy.push(c.eatPunct(".") ? { table: first, column: parseIdent(c) } : first);
    } while (c.eatPunct(","));
  }

  if (c.eatWord("HAVING")) filters.push(...parseFilterChain(c, true));

  let orderBy: SqlOrderBy[] | undefined;
  if (c.eatWord("ORDER")) {
    c.expectWord("BY");
    orderBy = [];
    do {
      const column = parseIdent(c);
      let direction: SqlOrderBy["direction"] = "asc";
      if (c.eatWord("DESC")) direction = "desc";
      else c.eatWord("ASC");
      orderBy.push({ column, direction });
    } while (c.eatPunct(","));
  }

  let limit: number | undefined;
  if (c.eatWord("LIMIT")) {
    const t = c.next();
    if (t.kind !== "number" || t.value === undefined || t.value < 0) throw new SqlParseError("bad LIMIT");
    limit = Math.floor(t.value);
  }

  if (!c.atEnd()) throw new SqlParseError(`trailing input: ${c.peek().text}`);

  const q: SqlBuilderQuery = { table, columns, filters, groupBy: groupBy ?? [] };
  if (orderBy) q.orderBy = orderBy;
  if (limit !== undefined && limit > 0) q.limit = limit;
  return q;
}

function parseSelectList(c: TokenCursor): SqlColumn[] {
  if (c.eatPunct("*")) {
    if (c.eatPunct(",")) throw new SqlParseError("* mixed with columns");
    return [];
  }
  const cols: SqlColumn[] = [];
  do {
    const { name, aggregation } = parseExpr(c);
    const col: SqlColumn = { name };
    if (aggregation) col.aggregation = aggregation;
    if (c.eatWord("AS")) col.alias = parseIdent(c);
    cols.push(col);
  } while (c.eatPunct(","));
  return cols;
}

/** One scalar/aggregate expression: `col`, `count()`, `count([DISTINCT] col)`, `math::agg(col)`. */
function parseExpr(c: TokenCursor): { name: string; aggregation?: SqlAggregation } {
  const t = c.peek();
  if (t.kind === "word" && t.text.toLowerCase() === "count" && c.peek(1).text === "(") {
    c.next();
    c.expectPunct("(");
    if (c.eatPunct(")")) return { name: "*", aggregation: "count" };
    const agg: SqlAggregation = c.eatWord("DISTINCT") ? "count_distinct" : "count";
    const name = parseIdent(c);
    c.expectPunct(")");
    return { name, aggregation: agg };
  }
  if (t.kind === "word" && t.text.toLowerCase() === "math" && c.peek(1).text === "::") {
    c.next();
    c.expectPunct("::");
    const fn = c.next();
    const agg = fn.kind === "word" ? MATH_AGGS[fn.text.toLowerCase()] : undefined;
    if (!agg) throw new SqlParseError(`unknown math:: function ${fn.text}`);
    c.expectPunct("(");
    const name = parseIdent(c);
    c.expectPunct(")");
    return { name, aggregation: agg };
  }
  return { name: parseIdent(c) };
}

function parseFilterChain(c: TokenCursor, aggregate: boolean): SqlFilter[] {
  const filters: SqlFilter[] = [];
  let logical: SqlLogical | undefined;
  for (;;) {
    filters.push(parseFilterTerm(c, aggregate, logical));
    if (c.eatWord("AND")) logical = "AND";
    else if (c.eatWord("OR")) logical = "OR";
    else return filters;
  }
}

function parseFilterTerm(c: TokenCursor, aggregate: boolean, logical: SqlLogical | undefined): SqlFilter {
  let column: string;
  let aggregation: SqlAggregation | undefined;
  if (aggregate) {
    const expr = parseExpr(c);
    if (!expr.aggregation) throw new SqlParseError("non-aggregate expression in HAVING");
    column = expr.name;
    aggregation = expr.aggregation;
  } else {
    column = parseIdent(c);
  }
  const f: SqlFilter = { column, operator: parseOperator(c) };
  if (logical) f.logical = logical;
  if (aggregate) {
    f.isAggregate = true;
    f.aggregation = aggregation;
  }
  if (f.operator !== "IS NULL" && f.operator !== "IS NOT NULL") f.value = parseValue(c);
  return f;
}

function parseOperator(c: TokenCursor): SqlOperator {
  if (c.eatWord("IS")) {
    if (c.eatWord("NOT")) {
      c.expectWord("NULL");
      return "IS NOT NULL";
    }
    c.expectWord("NULL");
    return "IS NULL";
  }
  if (c.eatWord("LIKE")) return "LIKE";
  for (const op of ["!=", ">=", "<=", "=", ">", "<"] as const) {
    if (c.eatPunct(op)) return op;
  }
  if (c.eatPunct("<>")) return "!=";
  throw new SqlParseError(`expected operator, got ${c.peek().text || "end"}`);
}

function parseValue(c: TokenCursor): string | number | boolean {
  const t = c.next();
  if (t.kind === "string") return t.text;
  if (t.kind === "number" && t.value !== undefined) return t.value;
  if (t.kind === "word" && t.upper === "TRUE") return true;
  if (t.kind === "word" && t.upper === "FALSE") return false;
  throw new SqlParseError(`expected literal, got ${t.text || "end"}`);
}

/** One bare identifier (SurrealQL never double-quotes in the emitter subset). */
function parseIdent(c: TokenCursor): string {
  const t = c.next();
  if (t.kind === "word" && !RESERVED.has(t.upper)) return t.text;
  throw new SqlParseError(`expected identifier, got ${t.text || "end"}`);
}
