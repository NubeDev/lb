// Pure merge of a datasource's discovered catalog into a designer record (schema-designer scope).
// Separated from the canvas so the import logic — neutral-type mapping, table dedup, layout seeding,
// and FK inference — is unit-testable with no React/IO. One responsibility, one file (FILE-LAYOUT).
//
// FK inference reuses the read-only ERD's `inferRelations` (`erd/schemaToFlow`): the SAME
// naming-convention guess (`<table>_id` / `<table>Ref`) the Discovery → Diagram draws, so an imported
// schema arrives with the same relationship lines. In the DESIGNER those become editable declared
// FKs (the author can delete/adjust), not the read-only ERD's dashed guesses.

import type { DbColumn, DbSchemaRecord, DesignFk } from "@/lib/datasources";
import { inferRelations } from "../erd/schemaToFlow";

/** One discovered table: its name + the live columns from `describeTable`. */
export interface DiscoveredTable {
  name: string;
  columns: DbColumn[];
}

/** Grid spacing for freshly-imported table nodes (px). */
const COL_W = 280;
const ROW_H = 220;
const GRID_COLS = 4;
const ORIGIN = 40;

/** Merge discovered tables into `record`: append tables not already present (by name), seed a grid
 *  layout for each new one (so nodes don't stack at {0,0}), and add inferred FKs (deduped against the
 *  record's existing FKs). Existing tables/positions/FKs are preserved. Pure + deterministic. */
export function mergeImport(record: DbSchemaRecord, discovered: DiscoveredTable[]): DbSchemaRecord {
  const importedTables = discovered.map((t) => ({
    name: t.name,
    pk: [] as string[],
    columns: t.columns.map((c) => ({
      name: c.name,
      type: guessNeutralType(c.dataType),
      nullable: c.nullable,
    })),
  }));
  const fresh = importedTables.filter((it) => !record.tables.some((rt) => rt.name === it.name));

  // Infer over the FULL merged table set so an imported child can reference an already-present parent.
  const inferInput = [...record.tables, ...fresh].map((t) => ({
    name: t.name,
    columns: t.columns.map((c) => ({ name: c.name, dataType: c.type, nullable: c.nullable })),
  }));
  const existingFk = new Set(
    record.fks.map((f) => `${f.fromTable}.${f.fromColumns[0]}->${f.toTable}`),
  );
  const inferredFks: DesignFk[] = inferRelations(inferInput)
    .filter((r) => !existingFk.has(`${r.source}.${r.sourceHandle}->${r.target}`))
    .map((r) => ({
      name: "",
      fromTable: r.source,
      fromColumns: [r.sourceHandle],
      toTable: r.target,
      toColumns: [r.targetHandle ?? "id"],
    }));

  const layout = { ...record.layout };
  const base = record.tables.length;
  fresh.forEach((it, i) => {
    const n = base + i;
    layout[it.name] = {
      x: ORIGIN + (n % GRID_COLS) * COL_W,
      y: ORIGIN + Math.floor(n / GRID_COLS) * ROW_H,
    };
  });

  return {
    ...record,
    tables: [...record.tables, ...fresh],
    fks: [...record.fks, ...inferredFks],
    layout,
  };
}

/** A loose guess at the neutral type from a live catalog type string (import-from-source). Maps
 *  common SQL type names back to the canonical vocabulary; unknown types default to `text`. */
export function guessNeutralType(liveType: string): string {
  const lc = liveType.toLowerCase();
  if (lc.includes("int")) return "integer";
  if (lc.includes("char") || lc.includes("text") || lc.includes("string")) return "text";
  if (lc.includes("float") || lc.includes("double") || lc.includes("real")) return "real";
  if (lc.includes("bool")) return "boolean";
  if (lc.includes("blob") || lc.includes("binary") || lc.includes("bytea")) return "blob";
  if (lc.includes("timestamp") || lc.includes("datetime")) return "timestamp";
  if (lc.includes("date")) return "date";
  if (lc.includes("numeric") || lc.includes("decimal")) return "numeric";
  if (lc.includes("json")) return "json";
  return "text";
}
