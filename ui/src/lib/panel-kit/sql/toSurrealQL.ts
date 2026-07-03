// The `SqlBuilderQuery` → SurrealQL string renderer (widget-builder Slice C, the analog of Grafana's
// SQL `expressionBuilder`). One file, one responsibility (FILE-LAYOUT): emit
// `SELECT … FROM … WHERE … GROUP BY … ORDER BY … LIMIT …` from the typed builder query. It only ever
// emits a SELECT — Builder mode has no syntax for a write, and whatever string this produces is still
// parse-allowlisted + bounded + workspace-walled by `store.query` at the host (the boundary).

import type { SqlAggregation, SqlBuilderQuery, SqlColumn, SqlFilter } from "./query";

/** Render an aggregation over a column name to SurrealQL (`count()`, `math::sum(col)`, …). SurrealDB
 *  spells the stats functions `math::sum`/`avg`/`min`/`max`; `count()` is a bare aggregate. */
function renderAggregation(agg: SqlAggregation, col: string): string {
  if (agg === "count") return col === "*" ? "count()" : `count(${col})`;
  return `math::${agg}(${col})`;
}

/** Render one SELECT column (with its optional aggregation, aliased to a stable name for the result). */
function renderColumn(c: SqlColumn): string {
  if (!c.aggregation) return c.name;
  const expr = renderAggregation(c.aggregation, c.name);
  // Alias so the result column has a predictable key the table/chart views can map (e.g. `avg_payload`).
  const alias = c.name === "*" ? c.aggregation : `${c.aggregation}_${c.name}`;
  return `${expr} AS ${alias}`;
}

/** Render a JS value as a SurrealQL literal (quote strings, pass numbers/bools through). Single quotes
 *  are escaped by doubling so a value can never break out of the literal. */
function renderValue(value: string | number | boolean): string {
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return `'${value.replace(/'/g, "''")}'`;
}

/** Render one WHERE filter (`column <op> value`). */
function renderFilter(f: SqlFilter): string {
  return `${f.column} ${f.operator} ${renderValue(f.value)}`;
}

/** Render a `SqlBuilderQuery` to a SurrealQL SELECT string. Returns `""` if no table is chosen yet
 *  (the builder is incomplete — the caller shows nothing to run). */
export function toSurrealQL(query: SqlBuilderQuery): string {
  if (!query.table.trim()) return "";

  const cols =
    query.columns.length > 0 ? query.columns.map(renderColumn).join(", ") : "*";
  let sql = `SELECT ${cols} FROM ${query.table}`;

  if (query.filters.length > 0) {
    sql += ` WHERE ${query.filters.map(renderFilter).join(" AND ")}`;
  }
  if (query.groupBy.length > 0) {
    sql += ` GROUP BY ${query.groupBy.join(", ")}`;
  }
  if (query.orderBy?.column) {
    sql += ` ORDER BY ${query.orderBy.column} ${query.orderBy.direction.toUpperCase()}`;
  }
  if (typeof query.limit === "number" && query.limit > 0) {
    sql += ` LIMIT ${Math.floor(query.limit)}`;
  }
  return sql;
}
