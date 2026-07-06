// The `SqlBuilderQuery` → standard-SQL string renderer (query-builder-common scope) — the
// federation analog of `toSurrealQL.ts`. One file, one responsibility (FILE-LAYOUT): emit
// `SELECT … FROM … WHERE … GROUP BY … ORDER BY … LIMIT …` from the typed builder query for an
// external SQL engine (sqlite / postgres / timescale). It only ever emits a SELECT — Builder mode
// has no syntax for a write, and whatever string this produces is still SELECT-validated +
// workspace-walled by `federation.query` at the host (the boundary).
//
// Dialect deltas from SurrealQL:
//   - Identifiers are DOUBLE-QUOTED (the `ident()` rule from `useDatasourceQuery.ts`: a mixed-case
//     or reserved-word column can never break out of the identifier position; embedded `"` is
//     doubled). SurrealQL uses bare lowercase identifiers; postgres folds unquoted to lowercase,
//     sqlite is permissive — double-quoting is the safe superset.
//   - Aggregates are ANSI (`COUNT(*)`, `SUM("col")`, `AVG("col")`, …) — NOT Surreal's `math::sum`
//     or bare `count()`.
//   - String literals use single quotes with `'` doubled (the same rule as SurrealQL).
//   - LIMIT is the ANSI `LIMIT n` (sqlite + postgres + timescale all speak it; OFFSET is a
//     deferred follow-up — the builder has no offset row).
//
// The v1 subset (SELECT/FROM/WHERE/GROUP BY/ORDER BY/LIMIT) is dialect-free across sqlite/postgres/
// timescale. A per-kind split (scope OQ #1) lands only when a real delta forces it — e.g. a
// time-bucket emit for the chart `time-series` format hint (postgres `date_trunc` / sqlite
// `strftime` / timescale `time_bucket`).

import type { SqlAggregation, SqlBuilderQuery, SqlColumn, SqlFilter } from "./query";

/** The ANSI aggregate spelling (`COUNT(*)`, `SUM(col)`, `AVG(col)`, …). `count` over `*` is the
 *  SQL standard `COUNT(*)`; anything else is `<UPPER>(col)`. */
function renderAggregation(agg: SqlAggregation, col: string): string {
  if (agg === "count") return col === "*" ? "COUNT(*)" : `COUNT(${ident(col)})`;
  return `${agg.toUpperCase()}(${ident(col)})`;
}

/** Render one SELECT column (with its optional aggregation, aliased to a stable name for the result). */
function renderColumn(c: SqlColumn): string {
  if (!c.aggregation) return ident(c.name);
  const expr = renderAggregation(c.aggregation, c.name);
  // Alias so the result column has a predictable key the table/chart views can map (e.g. `avg_payload`).
  const alias = c.name === "*" ? c.aggregation : `${c.aggregation}_${c.name}`;
  return `${expr} AS ${ident(alias)}`;
}

/** Double-quote a SQL identifier with embedded `"` doubled — a column/table name can never break
 *  out of the identifier position. Mirrors the shipped `ident()` helper in `useDatasourceQuery.ts`. */
function ident(name: string): string {
  return `"${name.replace(/"/g, '""')}"`;
}

/** Render a JS value as a SQL literal (quote strings with `'` doubled; pass numbers/bools through).
 *  Same rule as SurrealQL — single-quoted strings are ANSI. */
function renderValue(value: string | number | boolean): string {
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return `'${value.replace(/'/g, "''")}'`;
}

/** Render one WHERE filter (`"column" <op> value`). */
function renderFilter(f: SqlFilter): string {
  return `${ident(f.column)} ${f.operator} ${renderValue(f.value)}`;
}

/** Render a `SqlBuilderQuery` to a standard-SQL SELECT string. Returns `""` if no table is chosen
 *  yet (the builder is incomplete — the caller shows nothing to run). */
export function toStandardSql(query: SqlBuilderQuery): string {
  if (!query.table.trim()) return "";

  const cols =
    query.columns.length > 0 ? query.columns.map(renderColumn).join(", ") : "*";
  let sql = `SELECT ${cols} FROM ${ident(query.table)}`;

  if (query.filters.length > 0) {
    sql += ` WHERE ${query.filters.map(renderFilter).join(" AND ")}`;
  }
  if (query.groupBy.length > 0) {
    sql += ` GROUP BY ${query.groupBy.map(ident).join(", ")}`;
  }
  if (query.orderBy?.column) {
    sql += ` ORDER BY ${ident(query.orderBy.column)} ${query.orderBy.direction.toUpperCase()}`;
  }
  if (typeof query.limit === "number" && query.limit > 0) {
    sql += ` LIMIT ${Math.floor(query.limit)}`;
  }
  return sql;
}
