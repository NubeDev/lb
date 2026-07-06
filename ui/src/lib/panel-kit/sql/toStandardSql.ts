// The `SqlBuilderQuery` → standard-SQL string renderer (query-builder-common scope) — the
// federation analog of `toSurrealQL.ts`. One file, one responsibility (FILE-LAYOUT): emit
// `SELECT … FROM … [<joins>] WHERE … GROUP BY … HAVING … ORDER BY … LIMIT …` from the typed builder
// query for an external SQL engine (sqlite / postgres / timescale). It only ever emits a SELECT —
// Builder mode has no syntax for a write, and whatever string this produces is still SELECT-validated
// + workspace-walled by `federation.query` at the host (the boundary).
//
// visual-canvas-builder slice additions: ANSI JOINs (with composite ON keys + CROSS), per-column
// alias/table/order, WHERE-vs-HAVING split by `isAggregate`, AND/OR chaining by `logical`,
// `LIKE` + `IS NULL`/`IS NOT NULL`, qualified `{table, column}` group-by, and multi-column ORDER BY.
// Identifier qualification: when `query.joins` is non-empty every identifier renders as
// `"table"."column"`; with no joins it stays the bare `"column"` (back-compat — the pre-slice
// goldens are byte-identical).
//
// Dialect deltas from SurrealQL (unchanged):
//   - Identifiers are DOUBLE-QUOTED (the `ident()` rule: a mixed-case or reserved-word column can
//     never break out of the identifier position; embedded `"` is doubled).
//   - Aggregates are ANSI (`COUNT(*)`, `SUM("col")`, `AVG("col")`, `COUNT(DISTINCT "col")`, …).
//   - String literals use single quotes with `'` doubled.
//   - LIMIT is the ANSI `LIMIT n` (OFFSET deferred — the builder has no offset row).

import {
  isPendingJoin,
  normalizeOrderBy,
  type SqlAggregation,
  type SqlBuilderQuery,
  type SqlColumn,
  type SqlFilter,
  type SqlGroupByEntry,
  type SqlJoin,
  type SqlOrderBy,
} from "./query";

/** Double-quote a SQL identifier with embedded `"` doubled — a column/table name can never break
 *  out of the identifier position. Mirrors the shipped `ident()` helper in `useDatasourceQuery.ts`. */
function ident(name: string): string {
  return `"${name.replace(/"/g, '""')}"`;
}

/** Qualify a (table, column) pair as `"table"."column"`. */
function qualify(table: string, column: string): string {
  return `${ident(table)}.${ident(column)}`;
}

/** Render a JS value as a SQL literal (quote strings with `'` doubled; pass numbers/bools through).
 *  Same rule as SurrealQL — single-quoted strings are ANSI. Undefined ⇒ empty string literal. */
function renderValue(value: string | number | boolean | undefined): string {
  if (value === undefined) return "''";
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return `'${value.replace(/'/g, "''")}'`;
}

/** The ANSI aggregate expression for `agg` over an already-rendered column expression (`col`).
 *  `count_distinct` is `COUNT(DISTINCT col)`. The `count` over `*` special-case (`COUNT(*)`) is
 *  handled by the caller before reaching here. */
function aggregateExpr(agg: SqlAggregation, col: string): string {
  if (agg === "count_distinct") return `COUNT(DISTINCT ${col})`;
  if (agg === "count") return `COUNT(${col})`;
  return `${agg.toUpperCase()}(${col})`;
}

/** Stable-sort columns by `order` (missing = last), preserving input order within equal keys. */
function stableSortByOrder<T extends { order?: number }>(arr: T[]): T[] {
  return arr
    .map((item, idx) => ({ item, idx, key: item.order ?? Number.POSITIVE_INFINITY }))
    .sort((a, b) => a.key - b.key || a.idx - b.idx)
    .map((x) => x.item);
}

/** Render one SELECT column. `joined` ⇒ qualify identifiers with the owning table; the owning table
 *  is `c.table ?? fromTable`. Aggregated columns are aliased (`AS "alias"`) — alias falls back to
 *  `${aggregation}_${name}` (the historical default; `count` over `*` falls back to `count`).
 *  `count` over `*` always emits `COUNT(*)` (never `COUNT("*")`) — `*` is the SQL wildcard, not an
 *  identifier. */
function renderColumn(c: SqlColumn, joined: boolean, fromTable: string): string {
  if (c.aggregation === "count" && c.name === "*") {
    const alias = ident(c.alias ?? "count");
    return `COUNT(*) AS ${alias}`;
  }
  const table = c.table ?? fromTable;
  const colExpr = joined ? qualify(table, c.name) : ident(c.name);
  if (!c.aggregation) return c.alias ? `${colExpr} AS ${ident(c.alias)}` : colExpr;
  const expr = aggregateExpr(c.aggregation, colExpr);
  const alias = ident(c.alias ?? `${c.aggregation}_${c.name}`);
  return `${expr} AS ${alias}`;
}

/** Render one ANSI JOIN clause. `cross` emits `CROSS JOIN "t"` (no ON clause); otherwise
 *  `<TYPE> JOIN "table" ON <keys joined by AND>`. `leftTable` defaults to the FROM table. */
function renderJoin(j: SqlJoin, fromTable: string): string {
  const type = j.type.toUpperCase();
  if (j.type === "cross") {
    return `CROSS JOIN ${ident(j.table)}`;
  }
  const ons = (j.on ?? [])
    .map((k) => {
      const lt = k.leftTable ?? fromTable;
      return `${qualify(lt, k.leftColumn)} = ${qualify(j.table, k.rightColumn)}`;
    })
    .join(" AND ");
  return `${type} JOIN ${ident(j.table)} ON ${ons}`;
}

/** True if `op` is one of the value-less operators (`IS NULL` / `IS NOT NULL`). */
function isValueless(op: SqlFilter["operator"]): boolean {
  return op === "IS NULL" || op === "IS NOT NULL";
}

/** Render one filter's left-hand expression. For a HAVING filter with an `aggregation`, emit the
 *  aggregate expression (`AVG("t"."c")`) — NEVER the SELECT alias (ANSI/Postgres forbid it). For a
 *  plain filter (or a defensive HAVING filter with no aggregation), emit the bare qualified column. */
function renderFilterLhs(f: SqlFilter, joined: boolean, fromTable: string): string {
  const table = f.table ?? fromTable;
  const colExpr = joined ? qualify(table, f.column) : ident(f.column);
  if (f.isAggregate && f.aggregation) return aggregateExpr(f.aggregation, colExpr);
  return colExpr;
}

/** Render one filter as `<lhs> <op> [<value>]` (no value for IS NULL / IS NOT NULL). */
function renderFilter(f: SqlFilter, joined: boolean, fromTable: string): string {
  const lhs = renderFilterLhs(f, joined, fromTable);
  if (isValueless(f.operator)) return `${lhs} ${f.operator}`;
  return `${lhs} ${f.operator} ${renderValue(f.value)}`;
}

/** Render a WHERE or HAVING clause (selected by `aggregate`) — each filter chained to the previous
 *  by its `logical` (default `AND`). First filter carries no leading logical. Returns `""` if empty. */
function renderFilterClause(
  filters: SqlFilter[],
  joined: boolean,
  fromTable: string,
  aggregate: boolean,
): string {
  const selected = filters.filter((f) => (!!f.isAggregate) === aggregate);
  if (selected.length === 0) return "";
  return selected
    .map((f, i) => {
      const rendered = renderFilter(f, joined, fromTable);
      return i === 0 ? rendered : `${f.logical ?? "AND"} ${rendered}`;
    })
    .join(" ");
}

/** Render one GROUP BY entry. A bare string qualifies with the FROM table when joins are present
 *  (back-compat — the legacy single-table groupBy stays unqualified when there are no joins). */
function renderGroupByEntry(g: SqlGroupByEntry, joined: boolean, fromTable: string): string {
  if (typeof g === "string") return joined ? qualify(fromTable, g) : ident(g);
  return qualify(g.table, g.column);
}

/** Render one ORDER BY clause (`"col" ASC/DESC`, qualified under joins). */
function renderOrderBy(o: SqlOrderBy, joined: boolean, fromTable: string): string {
  const table = o.table ?? fromTable;
  const colExpr = joined ? qualify(table, o.column) : ident(o.column);
  return `${colExpr} ${o.direction.toUpperCase()}`;
}

/** Render a `SqlBuilderQuery` to a standard-SQL SELECT string. Returns `""` if no table is chosen
 *  yet (the builder is incomplete — the caller shows nothing to run). */
export function toStandardSql(query: SqlBuilderQuery): string {
  if (!query.table.trim()) return "";

  const fromTable = query.table;
  // Pending joins (a canvas table not yet wired) never emit — nor does anything referencing their
  // tables: a column/filter/sort on an unjoined table would be invalid SQL.
  const emittableJoins = (query.joins ?? []).filter((j) => !isPendingJoin(j));
  const pendingTables = new Set(
    (query.joins ?? []).filter(isPendingJoin).map((j) => j.table),
  );
  const joined = emittableJoins.length > 0;
  const notPending = (table?: string) => !table || !pendingTables.has(table);

  const selectable = query.columns.filter((c) => notPending(c.table));
  const cols =
    selectable.length > 0
      ? stableSortByOrder(selectable).map((c) => renderColumn(c, joined, fromTable))
      : ["*"];
  let sql = `SELECT ${cols.join(", ")} FROM ${ident(fromTable)}`;

  for (const j of emittableJoins) sql += ` ${renderJoin(j, fromTable)}`;

  const filters = query.filters.filter((f) => notPending(f.table));
  const whereClause = renderFilterClause(filters, joined, fromTable, false);
  if (whereClause) sql += ` WHERE ${whereClause}`;

  const groupBys = (query.groupBy ?? []).filter((g) => typeof g === "string" || notPending(g.table));
  if (groupBys.length > 0) {
    sql += ` GROUP BY ${groupBys.map((g) => renderGroupByEntry(g, joined, fromTable)).join(", ")}`;
  }

  const havingClause = renderFilterClause(filters, joined, fromTable, true);
  if (havingClause) sql += ` HAVING ${havingClause}`;

  const orderBys = normalizeOrderBy(query.orderBy)?.filter((o) => notPending(o.table));
  if (orderBys && orderBys.length > 0) {
    sql += ` ORDER BY ${orderBys.map((o) => renderOrderBy(o, joined, fromTable)).join(", ")}`;
  }

  if (typeof query.limit === "number" && query.limit > 0) {
    sql += ` LIMIT ${Math.floor(query.limit)}`;
  }
  return sql;
}
