// The schema-designer API client (schema-designer scope) — one call per export, mirroring the
// shipped host verbs 1:1 over the `mcp_call` bridge. The UI never calls `invoke` directly; it goes
// through these named verbs (FILE-LAYOUT frontend rules). Each is capability-gated server-side; the
// workspace + principal come from the session token (the wall, §7), never an argument.
//
// CRUD (`dbschema.*`) is store-only; write/migrate/export route to the supervised federation
// sidecar through the host's gated pipeline (resolve → net:* → mediate DSN → sidecar).

import type {
  DbSchemaRecord,
  DbSchemaSummary,
  ExportResult,
  MigrateResult,
  WriteResult,
} from "./dbschema.types";
import { invoke } from "@/lib/ipc/invoke";

/** The sidecar's snake_case shapes (decoded into the camelCase wire types below). */
interface RawSchemaRecord {
  name: string;
  version: number;
  tables: {
    name: string;
    columns: { name: string; type: string; nullable: boolean; default?: string }[];
    pk: string[];
  }[];
  fks: {
    name: string;
    from_table: string;
    from_columns: string[];
    to_table: string;
    to_columns: string[];
    on_delete?: string;
  }[];
  layout: Record<string, { x: number; y: number }>;
}

interface RawSummary {
  name: string;
  table_count: number;
  version: number;
}

interface RawMigrateStatement {
  kind: "create_table" | "add_column" | "add_fk";
  table: string;
  column?: string;
  name?: string;
  sql: string;
}

interface RawMigrateResult {
  statements: RawMigrateStatement[];
  applied: boolean;
  destructive_refusal?: string;
}

/** Decode the host's snake_case record into the camelCase `DbSchemaRecord` the UI works with. */
function toRecord(r: RawSchemaRecord): DbSchemaRecord {
  return {
    name: r.name,
    version: r.version,
    tables: r.tables.map((t) => ({
      name: t.name,
      columns: t.columns.map((c) => ({
        name: c.name,
        type: c.type,
        nullable: c.nullable,
        ...(c.default !== undefined ? { default: c.default } : {}),
      })),
      pk: t.pk,
    })),
    fks: r.fks.map((f) => ({
      name: f.name,
      fromTable: f.from_table,
      fromColumns: f.from_columns,
      toTable: f.to_table,
      toColumns: f.to_columns,
      ...(f.on_delete !== undefined ? { onDelete: f.on_delete } : {}),
    })),
    layout: Object.fromEntries(
      Object.entries(r.layout).map(([k, v]) => [k, { x: v.x, y: v.y }]),
    ),
  };
}

/** Encode the UI's camelCase record into the host's snake_case JSON for `dbschema.save`. */
export function encodeRecord(rec: DbSchemaRecord): RawSchemaRecord {
  return {
    name: rec.name,
    version: rec.version || 1,
    tables: rec.tables.map((t) => ({
      name: t.name,
      columns: t.columns.map((c) => ({
        name: c.name,
        type: c.type,
        nullable: c.nullable,
        ...(c.default !== undefined ? { default: c.default } : {}),
      })),
      pk: t.pk,
    })),
    fks: rec.fks.map((f) => ({
      name: f.name,
      from_table: f.fromTable,
      from_columns: f.fromColumns,
      to_table: f.toTable,
      to_columns: f.toColumns,
      ...(f.onDelete !== undefined ? { on_delete: f.onDelete } : {}),
    })),
    layout: Object.fromEntries(
      Object.entries(rec.layout).map(([k, v]) => [k, { x: v.x, y: v.y }]),
    ),
  };
}

/** Upsert the `db_schema` record under `name`. Mirrors `dbschema.save` (member-tier). */
export async function saveDbSchema(name: string, schema: DbSchemaRecord): Promise<void> {
  await invoke("mcp_call", {
    tool: "dbschema.save",
    args: { name, schema: encodeRecord(schema) },
  });
}

/** Read one `db_schema` record. `null` if absent (or tombstoned, or cross-tenant). */
export async function getDbSchema(name: string): Promise<DbSchemaRecord | null> {
  const r = await invoke<RawSchemaRecord | { found: boolean }>("mcp_call", {
    tool: "dbschema.get",
    args: { name },
  });
  if ("found" in r) return null;
  return toRecord(r);
}

/** List the workspace's designed schemas — name + table count per row. */
export async function listDbSchemas(): Promise<DbSchemaSummary[]> {
  const r = await invoke<{ schemas: RawSummary[] }>("mcp_call", {
    tool: "dbschema.list",
    args: {},
  });
  return r.schemas.map((s) => ({
    name: s.name,
    tableCount: s.table_count,
    version: s.version,
  }));
}

/** Tombstone a `db_schema` record. Never touches any live database. */
export async function deleteDbSchema(name: string): Promise<void> {
  await invoke("mcp_call", { tool: "dbschema.delete", args: { name } });
}

/** Plan + (optionally) apply a migrate of `schema` to `source`. `dryRun` defaults to true (the
 *  Ask gate — nothing applies unless explicitly opted in). Mirrors `federation.migrate`. */
export async function migrateSchema(
  source: string,
  schema: DbSchemaRecord,
  dryRun: boolean,
): Promise<MigrateResult> {
  const r = await invoke<RawMigrateResult>("mcp_call", {
    tool: "federation.migrate",
    args: { source, schema: encodeRecord(schema), dry_run: dryRun },
  });
  return {
    statements: r.statements.map((s) => ({
      kind: s.kind,
      table: s.table,
      sql: s.sql,
      ...(s.column !== undefined ? { column: s.column } : {}),
      ...(s.name !== undefined ? { name: s.name } : {}),
    })),
    applied: r.applied,
    ...(r.destructive_refusal !== undefined
      ? { destructiveRefusal: r.destructive_refusal }
      : {}),
  };
}

/** Bounded INSERT/UPSERT into `source.table`. Mirrors `federation.write`. */
export async function federationWrite(
  source: string,
  table: string,
  columns: string[],
  rows: unknown[][],
  key?: string[],
): Promise<WriteResult> {
  const r = await invoke<{ affected: number }>("mcp_call", {
    tool: "federation.write",
    args: { source, table, columns, rows, ...(key !== undefined ? { key } : {}) },
  });
  return { affected: r.affected };
}

/** Export platform series data to an external table (durable job). Mirrors `federation.export`. */
export async function federationExport(input: {
  source: string;
  series: string;
  table: string;
  jobId: string;
  columns?: string[];
  key?: string[];
  range?: number;
}): Promise<ExportResult> {
  const r = await invoke<{ job_id: string }>("mcp_call", {
    tool: "federation.export",
    args: {
      source: input.source,
      from: { series: input.series },
      table: input.table,
      job_id: input.jobId,
      ...(input.columns !== undefined ? { columns: input.columns } : {}),
      ...(input.key !== undefined ? { key: input.key } : {}),
      ...(input.range !== undefined ? { range: input.range } : {}),
    },
  });
  return { jobId: r.job_id };
}
