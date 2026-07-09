// The schema-designer wire types (schema-designer scope). The `db_schema` record is the product —
// tables/columns/PK/FK + canvas layout. Types are dialect-neutral (`text`/`integer`/…); the
// migrate planner maps them per-kind at apply time (one design, many targets).

/** One designed column. `type` is a canonical neutral type (text/integer/real/boolean/blob/date/
 *  timestamp/numeric/json), validated at `dbschema.save`. */
export interface DesignColumn {
  name: string;
  type: string;
  nullable: boolean;
  default?: string;
}

/** One designed table. `pk` names the primary-key column set (composite allowed). */
export interface DesignTable {
  name: string;
  columns: DesignColumn[];
  pk: string[];
}

/** One designed foreign key: `fromTable.fromColumns` → `toTable.toColumns`. */
export interface DesignFk {
  name: string;
  fromTable: string;
  fromColumns: string[];
  toTable: string;
  toColumns: string[];
  onDelete?: string;
}

/** Canvas geometry for one table node (rides the record so the picture survives reload). */
export interface LayoutPos {
  x: number;
  y: number;
}

/** The full designed-schema record — what `dbschema.save` persists + `dbschema.get` returns. */
export interface DbSchemaRecord {
  name: string;
  version: number;
  tables: DesignTable[];
  fks: DesignFk[];
  layout: Record<string, LayoutPos>;
}

/** A list row for `dbschema.list` — name + table count (no layout; a browse row). */
export interface DbSchemaSummary {
  name: string;
  tableCount: number;
  version: number;
}

/** The canonical neutral type vocabulary (mirrors the sidecar's `dialect::NEUTRAL_TYPES`). */
export const NEUTRAL_TYPES: readonly string[] = [
  "text",
  "integer",
  "real",
  "boolean",
  "blob",
  "date",
  "timestamp",
  "numeric",
  "json",
] as const;

/** One planned migrate statement, classified by action (the UI renders a DDL preview by kind). */
export interface MigrateStatement {
  kind: "create_table" | "add_column" | "add_fk";
  table: string;
  /** For `add_column` only. */
  column?: string;
  /** For `add_fk` only. */
  name?: string;
  sql: string;
}

/** The `federation.migrate` result — the planned statements + whether they were applied. */
export interface MigrateResult {
  statements: MigrateStatement[];
  applied: boolean;
  /** Present when the diff REFUSED a destructive change (the copy says what to do instead). */
  destructiveRefusal?: string;
}

/** The `federation.write` result — the affected row count. */
export interface WriteResult {
  affected: number;
}

/** The `federation.export` result — the durable job id (resume key). */
export interface ExportResult {
  jobId: string;
}
