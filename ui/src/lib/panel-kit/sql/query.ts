// The typed SQL builder query model (widget-builder Slice C, the Grafana `grafana-sql` model). The
// visual builder edits a `SqlBuilderQuery`; `toSurrealQL` (in `toSurrealQL.ts`) / `toStandardSql.ts`
// render it to the dialect string `store.query` / `federation.query` runs. The SQL source cell stores
// BOTH the raw string (what runs) AND, when in Builder mode, this `SqlBuilderQuery` — so reopening
// returns to the builder.
//
// Builder mode can ONLY express a SELECT (it has no syntax for a write); Code mode is still
// parse-allowlisted to a single SELECT by `store.query`. The builder is convenience; the host's parse
// gate + workspace wall + row cap are the boundary. One responsibility per file (FILE-LAYOUT): this is
// only the data shape + its enums.
//
// visual-canvas-builder slice: every addition is OPTIONAL so a pre-slice persisted cell reopens
// unchanged. New: joins, HAVING (`isAggregate` + `aggregation`), column aliases/tables/order, AND/OR
// `logical` chains, `LIKE`/`IS NULL`/`IS NOT NULL`, multi-column ORDER BY (array shape — read accepts
// the legacy single object), and the qualified `{table, column}` groupBy entry. `SqlSourceState`
// also carries an opaque `builderLayout` blob (React-Flow node positions) the emitter NEVER reads.

/** Which half of the Builder⇄Code editor is active (Grafana's `EditorMode`). */
export type SqlEditorMode = "builder" | "code";

/** The result shaping for the chosen view (Grafana's "Format: Table"). `table` passes rows through;
 *  `time-series` asserts a time column (maps onto the chart view). */
export type SqlFormat = "table" | "time-series";

/** A supported aggregation over a column (the visual builder's Column/Aggregation row).
 *  `count_distinct` renders `COUNT(DISTINCT col)` (standard) / `count(DISTINCT col)` (surreal). */
export type SqlAggregation =
  | "count"
  | "count_distinct"
  | "sum"
  | "avg"
  | "min"
  | "max";

/** One selected column. `name = "*"` with `aggregation = "count"` is `count()` / `COUNT(*)`. */
export interface SqlColumn {
  name: string;
  aggregation?: SqlAggregation;
  /** Result column name (`AS "alias"`). When absent and aggregated, the emitters fall back to
   *  `${aggregation}_${name}` (the historical default). */
  alias?: string;
  /** Which table this column belongs to — qualifies it as `"table"."column"` when `query.joins` is
   *  non-empty. Defaults to the FROM table. Ignored when there are no joins (back-compat). */
  table?: string;
  /** Position in SELECT (1-based; stable sort — missing order sorts last). */
  order?: number;
}

/** A comparison operator for a WHERE/HAVING filter. `LIKE` and `IS NULL`/`IS NOT NULL` are the
 *  visual-canvas-builder additions; `IN` is deferred (no value-list UI yet). */
export type SqlOperator =
  | "="
  | "!="
  | ">"
  | ">="
  | "<"
  | "<="
  | "LIKE"
  | "IS NULL"
  | "IS NOT NULL";

/** How a filter chains to the PREVIOUS one in its clause. Default `"AND"`. */
export type SqlLogical = "AND" | "OR";

/** One WHERE or HAVING filter. `value` is OPTIONAL — `IS NULL` / `IS NOT NULL` carry none.
 *  `isAggregate: true` ⇒ the filter emits into HAVING (the aggregate expression, never the SELECT
 *  alias — ANSI/Postgres forbid aliases in HAVING); `false`/absent ⇒ WHERE. */
export interface SqlFilter {
  column: string;
  /** Which table this column belongs to — qualifies under joins. Defaults to the FROM table. */
  table?: string;
  operator: SqlOperator;
  /** A JS value rendered as a SQL literal. OPTIONAL — `IS NULL`/`IS NOT NULL` carry none. */
  value?: string | number | boolean;
  /** How this filter joins the previous one in its clause. Default `"AND"`. */
  logical?: SqlLogical;
  /** `true` ⇒ this filter emits into HAVING (with `aggregation`), not WHERE. */
  isAggregate?: boolean;
  /** REQUIRED when `isAggregate: true` — HAVING emits the aggregate expression, NEVER the SELECT alias. */
  aggregation?: SqlAggregation;
}

/** A supported ANSI JOIN type. `cross` carries no `on` clause. */
export type SqlJoinType = "inner" | "left" | "right" | "full" | "cross";

/** One half of a JOIN ON — the left column (`leftTable.leftColumn`) equals the joined table's
 *  `rightColumn`. `leftTable` defaults to the FROM table; set it explicitly when the left side of
 *  join N is a table joined earlier (not the FROM table). */
export interface SqlJoinKey {
  leftTable?: string;
  leftColumn: string;
  rightColumn: string;
}

/** One JOIN clause. `on` is usually one key; the array allows composite joins. OPTIONAL/empty for
 *  `cross` — CROSS JOIN has no ON clause (the emitter omits it). */
export interface SqlJoin {
  /** The joined (right) table. */
  table: string;
  type: SqlJoinType;
  on?: SqlJoinKey[];
}

/** True if `j` is a PENDING join — a canvas table dropped but not yet wired column-to-column
 *  (`on` empty on a non-cross type). A pending join is view-only: the emitters skip it (and
 *  anything referencing its table) so the SQL never shows a table without a join path; the canvas
 *  marks its node "not joined". Only an explicit `cross` legitimately carries no ON clause. */
export function isPendingJoin(j: SqlJoin): boolean {
  return j.type !== "cross" && (!j.on || j.on.length === 0);
}

/** A GROUP BY entry. A bare `string` means a column of the FROM table (back-compat); the object form
 *  `{table, column}` qualifies it (needed once joins are present — ambiguous names across joined
 *  tables are inevitable). */
export type SqlGroupByEntry = string | { table: string; column: string };

/** An ORDER BY clause. `table` qualifies under joins (defaults to the FROM table). */
export interface SqlOrderBy {
  column: string;
  table?: string;
  direction: "asc" | "desc";
}

/** Normalize a `SqlBuilderQuery.orderBy` (which may be the legacy single-object shape on read or the
 *  new array shape) into a flat array, or `null` if there is none. The WRITE shape is always the
 *  array; this helper exists because persisted pre-slice cells still carry the single object. */
export function normalizeOrderBy(orderBy?: SqlOrderBy | SqlOrderBy[]): SqlOrderBy[] | null {
  if (!orderBy) return null;
  if (Array.isArray(orderBy)) return orderBy.filter((o) => o?.column);
  return orderBy.column ? [orderBy] : null;
}

/** The typed visual-builder query (Grafana's `SQLExpression`/`SQLQuery` analog). Every addition is
 *  OPTIONAL — a pre-slice persisted cell reopens unchanged. */
export interface SqlBuilderQuery {
  /** The FROM (primary) table. */
  table: string;
  /** JOIN clauses (standard dialect only — the surreal emitter drops them defensively). */
  joins?: SqlJoin[];
  /** The SELECT columns (empty ⇒ `SELECT *`). */
  columns: SqlColumn[];
  /** The filters — split WHERE vs HAVING by `isAggregate`; chained by `logical` (default AND). */
  filters: SqlFilter[];
  /** The GROUP BY entries. A bare string means a FROM-table column (back-compat); `{table, column}`
   *  qualifies. Optional on write; the empty array keeps legacy `.groupBy` reads working. */
  groupBy?: SqlGroupByEntry[];
  /** The ORDER BY clauses. WRITE the array shape; READ accepts the legacy single object
   *  (normalized via `normalizeOrderBy`). */
  orderBy?: SqlOrderBy | SqlOrderBy[];
  /** The LIMIT (the host caps it regardless). */
  limit?: number;
}

/** A fresh, empty builder query for `table` — `SELECT * FROM table`. Keeps `groupBy: []` so existing
 *  reads of `.groupBy` (e.g. `.length`) still work on a fresh query. */
export function emptyQuery(table = ""): SqlBuilderQuery {
  return { table, columns: [], filters: [], groupBy: [] };
}

/** The full SQL source state a cell stores: the editor mode, the raw SQL string (what runs), the
 *  builder query (when authored in Builder mode), the format, and an opaque canvas-layout blob. */
export interface SqlSourceState {
  mode: SqlEditorMode;
  /** The raw SQL string — the source of truth the host runs. */
  rawSql: string;
  /** The builder query, present iff the source was last edited in Builder mode (so reopening returns). */
  builder?: SqlBuilderQuery;
  format: SqlFormat;
  /** Opaque React-Flow node positions (`{ [table]: {x,y} }`) — never read by `emitSql`; the canvas
   *  projection (`canvasModel.toFlow`) consumes it to restore a saved diagram. */
  builderLayout?: unknown;
}

/** A fresh SQL source state — Builder mode, empty query, table format. */
export function emptySqlSource(): SqlSourceState {
  return { mode: "builder", rawSql: "", builder: emptyQuery(), format: "table" };
}
