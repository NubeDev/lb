// The `SqlBuilderQuery` → SurrealQL string renderer (widget-builder Slice C, the analog of Grafana's
// SQL `expressionBuilder`). One file, one responsibility (FILE-LAYOUT): emit
// `SELECT … FROM … WHERE … GROUP BY … HAVING … ORDER BY … LIMIT …` from the typed builder query. It
// only ever emits a SELECT — Builder mode has no syntax for a write, and whatever string this
// produces is still parse-allowlisted + bounded + workspace-walled by `store.query` at the host
// (the boundary).
//
// visual-canvas-builder slice additions: HAVING (`isAggregate`), per-column aliases, multi-column
// ORDER BY, AND/OR chaining, `LIKE` + `IS NULL`/`IS NOT NULL`. SurrealQL has no ANSI `JOIN … ON` —
// when `query.joins` is non-empty the emitter DROPS them (defensive; the UI gates the join
// affordance on `dialect === "standard"` so this never runs in normal use, but a model can still
// carry joins and we must never emit invalid SurrealQL). Record-link joins are a named follow-up.

import {
  normalizeOrderBy,
  type SqlAggregation,
  type SqlBuilderQuery,
  type SqlColumn,
  type SqlFilter,
  type SqlGroupByEntry,
  type SqlOrderBy,
} from "./query";

/** Render an aggregation over a column name to SurrealQL (`count()`, `count(DISTINCT col)`,
 *  `math::sum(col)`, …). SurrealDB spells the stats functions `math::sum`/`avg`/`min`/`max`; `count()`
 *  is a bare aggregate and `count_distinct` is `count(DISTINCT col)` (close to ANSI; valid SurrealQL). */
function aggregateExpr(agg: SqlAggregation, col: string): string {
  if (agg === "count") return col === "*" ? "count()" : `count(${col})`;
  if (agg === "count_distinct") return `count(DISTINCT ${col})`;
  return `math::${agg}(${col})`;
}

/** Render one SELECT column (with its optional aggregation, aliased to a stable name for the result).
 *  Bare identifiers (SurrealQL uses bare lowercase). Aliases render bare too (no quotes — existing). */
function renderColumn(c: SqlColumn): string {
  if (c.aggregation === "count" && c.name === "*") {
    const alias = c.alias ?? "count";
    return `count() AS ${alias}`;
  }
  if (!c.aggregation) return c.alias ? `${c.name} AS ${c.alias}` : c.name;
  const expr = aggregateExpr(c.aggregation, c.name);
  const alias = c.alias ?? (c.aggregation === "count" && c.name === "*" ? "count" : `${c.aggregation}_${c.name}`);
  return `${expr} AS ${alias}`;
}

/** Render a JS value as a SurrealQL literal (quote strings, pass numbers/bools through). Single quotes
 *  are escaped by doubling so a value can never break out of the literal. Undefined ⇒ empty literal. */
function renderValue(value: string | number | boolean | undefined): string {
  if (value === undefined) return "''";
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return `'${value.replace(/'/g, "''")}'`;
}

/** True if `op` is one of the value-less operators (`IS NULL` / `IS NOT NULL`). */
function isValueless(op: SqlFilter["operator"]): boolean {
  return op === "IS NULL" || op === "IS NOT NULL";
}

/** Render one filter's left-hand expression. For a HAVING filter with an `aggregation`, emit the
 *  aggregate expression (`math::avg(c)`) — NEVER the SELECT alias. */
function renderFilterLhs(f: SqlFilter): string {
  if (f.isAggregate && f.aggregation) return aggregateExpr(f.aggregation, f.column);
  return f.column;
}

/** Render one filter as `<lhs> <op> [<value>]` (no value for IS NULL / IS NOT NULL). */
function renderFilter(f: SqlFilter): string {
  const lhs = renderFilterLhs(f);
  if (isValueless(f.operator)) return `${lhs} ${f.operator}`;
  return `${lhs} ${f.operator} ${renderValue(f.value)}`;
}

/** Render a WHERE or HAVING clause (selected by `aggregate`) — each filter chained to the previous
 *  by its `logical` (default `AND`). Returns `""` if empty. */
function renderFilterClause(filters: SqlFilter[], aggregate: boolean): string {
  const selected = filters.filter((f) => (!!f.isAggregate) === aggregate);
  if (selected.length === 0) return "";
  return selected
    .map((f, i) => {
      const rendered = renderFilter(f);
      return i === 0 ? rendered : `${f.logical ?? "AND"} ${rendered}`;
    })
    .join(" ");
}

/** Render one GROUP BY entry. A bare string stays bare (a column of the FROM table). The object form
 *  emits `table.column` (SurrealQL's field-access — the `table` is the FROM table since surreal has
 *  no joins). */
function renderGroupByEntry(g: SqlGroupByEntry): string {
  if (typeof g === "string") return g;
  return `${g.table}.${g.column}`;
}

/** Render one ORDER BY clause (`col ASC/DESC`, bare). */
function renderOrderBy(o: SqlOrderBy): string {
  return `${o.column} ${o.direction.toUpperCase()}`;
}

/** Render a `SqlBuilderQuery` to a SurrealQL SELECT string. Returns `""` if no table is chosen yet
 *  (the builder is incomplete — the caller shows nothing to run). Joins, if present, are DROPPED
 *  (defensive — never emit invalid SurrealQL). */
export function toSurrealQL(query: SqlBuilderQuery): string {
  if (!query.table.trim()) return "";

  const cols =
    query.columns.length > 0 ? query.columns.map(renderColumn).join(", ") : "*";
  let sql = `SELECT ${cols} FROM ${query.table}`;

  const whereClause = renderFilterClause(query.filters, false);
  if (whereClause) sql += ` WHERE ${whereClause}`;

  const groupBys = query.groupBy ?? [];
  if (groupBys.length > 0) {
    sql += ` GROUP BY ${groupBys.map(renderGroupByEntry).join(", ")}`;
  }

  const havingClause = renderFilterClause(query.filters, true);
  if (havingClause) sql += ` HAVING ${havingClause}`;

  const orderBys = normalizeOrderBy(query.orderBy);
  if (orderBys && orderBys.length > 0) {
    sql += ` ORDER BY ${orderBys.map(renderOrderBy).join(", ")}`;
  }

  if (typeof query.limit === "number" && query.limit > 0) {
    sql += ` LIMIT ${Math.floor(query.limit)}`;
  }
  return sql;
}
