// The typed SQL builder query model (widget-builder Slice C, the Grafana `grafana-sql` model). The
// visual builder edits a `SqlBuilderQuery`; `toSurrealQL` (in `toSurrealQL.ts`) renders it to the
// SurrealQL string `store.query` runs. The SQL source cell stores BOTH the raw string (what runs) AND,
// when in Builder mode, this `SqlBuilderQuery` — so reopening returns to the builder.
//
// Builder mode can ONLY express a SELECT (it has no syntax for a write); Code mode is still
// parse-allowlisted to a single SELECT by `store.query`. The builder is convenience; the host's parse
// gate + workspace wall + row cap are the boundary. One responsibility per file (FILE-LAYOUT): this is
// only the data shape + its enums.

/** Which half of the Builder⇄Code editor is active (Grafana's `EditorMode`). */
export type SqlEditorMode = "builder" | "code";

/** The result shaping for the chosen view (Grafana's "Format: Table"). `table` passes rows through;
 *  `time-series` asserts a time column (maps onto the chart view). */
export type SqlFormat = "table" | "time-series";

/** A supported aggregation over a column (the visual builder's Column/Aggregation row). */
export type SqlAggregation = "count" | "sum" | "avg" | "min" | "max";

/** One selected column, optionally aggregated. `name = "*"` with `aggregation = "count"` is `count()`. */
export interface SqlColumn {
  name: string;
  aggregation?: SqlAggregation;
}

/** A comparison operator for a WHERE filter. */
export type SqlOperator = "=" | "!=" | ">" | ">=" | "<" | "<=";

/** One WHERE filter — `column <op> value`. `value` is a JS value rendered as a SurrealQL literal. */
export interface SqlFilter {
  column: string;
  operator: SqlOperator;
  value: string | number | boolean;
}

/** An ORDER BY clause. */
export interface SqlOrderBy {
  column: string;
  direction: "asc" | "desc";
}

/** The typed visual-builder query (Grafana's `SQLExpression`/`SQLQuery` analog). */
export interface SqlBuilderQuery {
  /** The FROM table (from `store.schema`'s table list). */
  table: string;
  /** The SELECT columns (empty ⇒ `SELECT *`). */
  columns: SqlColumn[];
  /** The WHERE filters (ANDed). */
  filters: SqlFilter[];
  /** The GROUP BY columns. */
  groupBy: string[];
  /** The ORDER BY (single column, like Grafana's default). */
  orderBy?: SqlOrderBy;
  /** The LIMIT (the host caps it regardless). */
  limit?: number;
}

/** A fresh, empty builder query for `table` — `SELECT * FROM table`. */
export function emptyQuery(table = ""): SqlBuilderQuery {
  return { table, columns: [], filters: [], groupBy: [] };
}

/** The full SQL source state a cell stores: the editor mode, the raw SurrealQL string (what
 *  `store.query` runs), the builder query (when authored in Builder mode), and the format. */
export interface SqlSourceState {
  mode: SqlEditorMode;
  /** The raw SurrealQL string — the source of truth `store.query` runs. */
  rawSql: string;
  /** The builder query, present iff the source was last edited in Builder mode (so reopening returns). */
  builder?: SqlBuilderQuery;
  format: SqlFormat;
}
