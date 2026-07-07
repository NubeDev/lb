// The standard-SQL → `SqlBuilderQuery` parser (query-builder slice-1 follow-up: Code→Builder sync) —
// the inverse of `toStandardSql.ts`. One responsibility (FILE-LAYOUT): recognize exactly the SELECT
// subset the builder model can express and return the typed query, or `null` when the SQL is not
// expressible (subquery, CTE, window fn, DISTINCT select, multi-statement, or anything unparseable).
// The parser produces a model — it NEVER becomes a second source of truth (the model-as-truth
// invariant, visual-canvas-builder scope): the caller decides what to do with the result, and the
// emitters remain the only SQL writers.
//
// Round-trip contract (pinned in `fromSql.roundtrip.test.ts`): for every query `q` the emitter
// goldens cover, `parseStandardSql(toStandardSql(q))` is semantically equal to `q` — i.e. re-emitting
// the parsed model reproduces the same SQL string. Hand-written variants are accepted where cheap:
// bare (unquoted) identifiers, `<>` (normalized to `!=`), `JOIN`/`LEFT OUTER JOIN` spellings, a
// trailing `;`, arbitrary whitespace, and any keyword case.

import {
  type SqlAggregation,
  type SqlBuilderQuery,
  type SqlColumn,
  type SqlFilter,
  type SqlGroupByEntry,
  type SqlJoin,
  type SqlJoinKey,
  type SqlJoinType,
  type SqlLogical,
  type SqlOperator,
  type SqlOrderBy,
} from "./query";
import { SqlParseError, TokenCursor, tokenize } from "./sqlTokens";

/** Keywords that can never be a bare identifier — stops the identifier rule from swallowing the next
 *  clause when the SQL uses unquoted names. */
const RESERVED = new Set([
  "SELECT", "FROM", "WHERE", "GROUP", "HAVING", "ORDER", "LIMIT", "BY", "AS", "ON", "AND", "OR",
  "INNER", "LEFT", "RIGHT", "FULL", "OUTER", "CROSS", "JOIN", "IS", "NULL", "NOT", "LIKE", "IN",
  "ASC", "DESC", "DISTINCT", "TRUE", "FALSE", "UNION", "WITH", "OVER",
]);

const AGGREGATES: Record<string, SqlAggregation> = {
  COUNT: "count",
  SUM: "sum",
  AVG: "avg",
  MIN: "min",
  MAX: "max",
};

/** A parsed column reference — `"t"."c"`, `t.c`, `"c"`, or `c`. */
interface ColRef {
  table?: string;
  column: string;
}

/** Parse a standard-SQL SELECT into the typed builder query, or `null` when the statement uses
 *  anything the model cannot express. Never throws. */
export function parseStandardSql(sql: string): SqlBuilderQuery | null {
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
  const from = parseTableRef(c);
  const rawJoins = parseRawJoins(c);

  const filters: SqlFilter[] = [];
  if (c.eatWord("WHERE")) filters.push(...parseFilterChain(c, false));

  let groupBy: SqlGroupByEntry[] | undefined;
  if (c.eatWord("GROUP")) {
    c.expectWord("BY");
    groupBy = parseGroupBy(c);
  }

  if (c.eatWord("HAVING")) filters.push(...parseFilterChain(c, true));

  let orderBy: SqlOrderBy[] | undefined;
  if (c.eatWord("ORDER")) {
    c.expectWord("BY");
    orderBy = parseOrderBy(c);
  }

  let limit: number | undefined;
  if (c.eatWord("LIMIT")) {
    const t = c.next();
    if (t.kind !== "number" || t.value === undefined || t.value < 0) throw new SqlParseError("bad LIMIT");
    limit = Math.floor(t.value);
  }

  if (!c.atEnd()) throw new SqlParseError(`trailing input: ${c.peek().text}`);

  // Resolve table ALIASES to real table names — the model has no alias concept, so `FROM site s …
  // s.name` becomes `{table:"site", column:"name"}`. A self-join (the same table under two aliases)
  // cannot survive this and fails ON orientation below ⇒ null (not expressible).
  const aliases = new Map<string, string>();
  if (from.alias) aliases.set(from.alias, from.table);
  for (const j of rawJoins) if (j.alias) aliases.set(j.alias, j.table);
  const resolve = (t?: string) => (t === undefined ? undefined : (aliases.get(t) ?? t));
  const table = from.table;

  for (const col of columns) col.table = resolve(col.table);
  for (const f of filters) f.table = resolve(f.table);
  const resolvedGroupBy = (groupBy ?? []).map((g) =>
    typeof g === "string" ? g : { table: resolve(g.table)!, column: g.column },
  );
  if (orderBy) for (const o of orderBy) o.table = resolve(o.table);
  const joins: SqlJoin[] = rawJoins.map((j) => {
    if (j.type === "cross") return { table: j.table, type: j.type };
    const on = j.on.map(([a, b]) =>
      toJoinKey(
        { table: resolve(a.table), column: a.column },
        { table: resolve(b.table), column: b.column },
        j.table,
        table,
      ),
    );
    return { table: j.table, type: j.type, on };
  });

  const q: SqlBuilderQuery = { table, columns: columns.map(stripUndefinedTable), filters: filters.map(stripUndefinedTable), groupBy: resolvedGroupBy };
  if (joins.length > 0) q.joins = joins;
  if (orderBy) q.orderBy = orderBy.map(stripUndefinedTable);
  if (limit !== undefined && limit > 0) q.limit = limit;
  return q;
}

/** Drop a `table: undefined` key left by alias resolution so parsed objects deep-equal the
 *  hand-built fixtures (an absent key, not an undefined one). */
function stripUndefinedTable<T extends { table?: string }>(x: T): T {
  if (x.table === undefined) {
    const { table: _drop, ...rest } = x;
    return rest as T;
  }
  return x;
}

/** A table reference with an optional alias: `t`, `t a`, `t AS a` (quoted or bare either side). */
function parseTableRef(c: TokenCursor): { table: string; alias?: string } {
  const table = parseIdent(c);
  if (c.eatWord("AS")) return { table, alias: parseIdent(c) };
  const t = c.peek();
  if (t.kind === "qident" || (t.kind === "word" && !RESERVED.has(t.upper))) {
    c.next();
    return { table, alias: t.text };
  }
  return { table };
}

/** SELECT list: a sole `*` (⇒ empty columns), or comma-separated column/aggregate expressions.
 *  `SELECT DISTINCT` and a `*` mixed with named columns are not expressible ⇒ throw. */
function parseSelectList(c: TokenCursor): SqlColumn[] {
  if (c.peek().kind === "word" && c.peek().upper === "DISTINCT") throw new SqlParseError("SELECT DISTINCT");
  if (c.eatPunct("*")) {
    if (c.eatPunct(",")) throw new SqlParseError("* mixed with columns");
    return [];
  }
  const cols: SqlColumn[] = [];
  do {
    cols.push(parseSelectItem(c));
  } while (c.eatPunct(","));
  return cols;
}

function parseSelectItem(c: TokenCursor): SqlColumn {
  const t = c.peek();
  // Aggregate: COUNT/SUM/AVG/MIN/MAX '(' [DISTINCT] (* | colref) ')'
  if (t.kind === "word" && AGGREGATES[t.upper] && c.peek(1).text === "(") {
    let agg = AGGREGATES[t.upper];
    c.next();
    c.expectPunct("(");
    let name: string;
    let table: string | undefined;
    if (c.eatPunct("*")) {
      if (agg !== "count") throw new SqlParseError(`${agg}(*)`);
      name = "*";
    } else {
      if (c.eatWord("DISTINCT")) {
        if (agg !== "count") throw new SqlParseError("DISTINCT under non-count");
        agg = "count_distinct";
      }
      const ref = parseColRef(c);
      name = ref.column;
      table = ref.table;
    }
    c.expectPunct(")");
    const col: SqlColumn = { name, aggregation: agg };
    if (table) col.table = table;
    const alias = parseOptionalAlias(c);
    if (alias !== undefined) col.alias = alias;
    return col;
  }
  const ref = parseColRef(c);
  const col: SqlColumn = { name: ref.column };
  if (ref.table) col.table = ref.table;
  const alias = parseOptionalAlias(c);
  if (alias !== undefined) col.alias = alias;
  return col;
}

/** `AS <ident>` after a select expression. A bare-word implicit alias is NOT accepted (ambiguous
 *  with the emitter subset's clause keywords; the emitters always write `AS`). */
function parseOptionalAlias(c: TokenCursor): string | undefined {
  if (!c.eatWord("AS")) return undefined;
  return parseIdent(c);
}

/** One JOIN clause as written, before alias resolution — ON keys as raw `[left, right]` refs. */
interface RawJoin {
  table: string;
  alias?: string;
  type: SqlJoinType;
  on: Array<[ColRef, ColRef]>;
}

/** JOIN clauses. Accepts `INNER|LEFT|RIGHT|FULL [OUTER] JOIN … ON k [AND k]*`, bare `JOIN` (inner),
 *  and `CROSS JOIN` (no ON), each with an optional table alias. Orientation of the ON equalities
 *  happens AFTER alias resolution (in `parse`) — here they are kept as written. */
function parseRawJoins(c: TokenCursor): RawJoin[] {
  const joins: RawJoin[] = [];
  for (;;) {
    let type: SqlJoinType | null = null;
    if (c.eatWord("INNER")) type = "inner";
    else if (c.eatWord("LEFT")) type = "left";
    else if (c.eatWord("RIGHT")) type = "right";
    else if (c.eatWord("FULL")) type = "full";
    else if (c.eatWord("CROSS")) type = "cross";
    else if (c.peek().upper === "JOIN") type = "inner";
    if (type === null) return joins;
    c.eatWord("OUTER");
    c.expectWord("JOIN");
    const ref = parseTableRef(c);
    if (type === "cross") {
      joins.push({ ...ref, type, on: [] });
      continue;
    }
    c.expectWord("ON");
    const on: Array<[ColRef, ColRef]> = [];
    do {
      const a = parseColRef(c);
      c.expectPunct("=");
      on.push([a, parseColRef(c)]);
    } while (c.eatWord("AND"));
    joins.push({ ...ref, type, on });
  }
}

/** Orient one `a = b` ON equality: the side qualified with the joined table is the right key; an
 *  unqualified side is assumed to be the joined table's column when the other side is qualified.
 *  Both sides on the joined table (or neither attributable) is not expressible. */
function toJoinKey(a: ColRef, b: ColRef, joinTable: string, fromTable: string): SqlJoinKey {
  const aRight = a.table === joinTable;
  const bRight = b.table === joinTable;
  if (aRight === bRight) {
    // Neither (or both) explicitly on the joined table: allow the `left = unqualified-right` shape.
    if (!aRight && a.table && !b.table) return key(a, b.column, fromTable);
    if (!bRight && b.table && !a.table) return key(b, a.column, fromTable);
    throw new SqlParseError("ON key does not reference the joined table");
  }
  const right = aRight ? a : b;
  const left = aRight ? b : a;
  return key(left, right.column, fromTable);
}

function key(left: ColRef, rightColumn: string, fromTable: string): SqlJoinKey {
  const k: SqlJoinKey = { leftColumn: left.column, rightColumn };
  if (left.table && left.table !== fromTable) k.leftTable = left.table;
  return k;
}

/** A WHERE/HAVING chain: `term ((AND|OR) term)*`. Parenthesized boolean groups are not expressible.
 *  In HAVING (`aggregate`), each term's lhs must be an aggregate expression; in WHERE it must not. */
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
  let ref: ColRef;
  let aggregation: SqlAggregation | undefined;
  const t = c.peek();
  if (t.kind === "word" && AGGREGATES[t.upper] && c.peek(1).text === "(") {
    if (!aggregate) throw new SqlParseError("aggregate expression in WHERE");
    aggregation = AGGREGATES[t.upper];
    c.next();
    c.expectPunct("(");
    if (c.eatWord("DISTINCT")) {
      if (aggregation !== "count") throw new SqlParseError("DISTINCT under non-count");
      aggregation = "count_distinct";
    }
    ref = parseColRef(c);
    c.expectPunct(")");
  } else {
    if (aggregate) throw new SqlParseError("non-aggregate expression in HAVING");
    ref = parseColRef(c);
  }

  const f: SqlFilter = { column: ref.column, operator: parseOperator(c) };
  if (ref.table) f.table = ref.table;
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

/** A comparison rhs: a string/number literal or TRUE/FALSE. A column rhs or expression is not
 *  expressible (the model's `value` is a literal). */
function parseValue(c: TokenCursor): string | number | boolean {
  const t = c.next();
  if (t.kind === "string") return t.text;
  if (t.kind === "number" && t.value !== undefined) return t.value;
  if (t.kind === "word" && t.upper === "TRUE") return true;
  if (t.kind === "word" && t.upper === "FALSE") return false;
  throw new SqlParseError(`expected literal, got ${t.text || "end"}`);
}

/** GROUP BY entries: an unqualified column stays a bare string (FROM-table column, back-compat);
 *  a qualified `t.c` becomes the `{table, column}` object form. */
function parseGroupBy(c: TokenCursor): SqlGroupByEntry[] {
  const out: SqlGroupByEntry[] = [];
  do {
    const ref = parseColRef(c);
    out.push(ref.table ? { table: ref.table, column: ref.column } : ref.column);
  } while (c.eatPunct(","));
  return out;
}

function parseOrderBy(c: TokenCursor): SqlOrderBy[] {
  const out: SqlOrderBy[] = [];
  do {
    const ref = parseColRef(c);
    let direction: SqlOrderBy["direction"] = "asc";
    if (c.eatWord("DESC")) direction = "desc";
    else c.eatWord("ASC");
    const o: SqlOrderBy = { column: ref.column, direction };
    if (ref.table) o.table = ref.table;
    out.push(o);
  } while (c.eatPunct(","));
  return out;
}

/** A column reference: `ident [. ident]` — quoted or bare. Qualified ⇒ `{table, column}`. */
function parseColRef(c: TokenCursor): ColRef {
  const first = parseIdent(c);
  if (c.eatPunct(".")) return { table: first, column: parseIdent(c) };
  return { column: first };
}

/** One identifier: a `"quoted"` name (any content) or a bare word that is not reserved. */
function parseIdent(c: TokenCursor): string {
  const t = c.next();
  if (t.kind === "qident") return t.text;
  if (t.kind === "word" && !RESERVED.has(t.upper)) return t.text;
  throw new SqlParseError(`expected identifier, got ${t.text || "end"}`);
}
