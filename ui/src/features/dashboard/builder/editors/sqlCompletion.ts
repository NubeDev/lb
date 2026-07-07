// Schema→CodeMirror-lang-sql completion projection (query-builder 10x, slice 2).
// One responsibility: project our `Schema` into `@codemirror/lang-sql`'s
// `SQLConfig` completion shape (a SQLNamespace mapping table names to their column
// names) + pick the CodeMirror dialect for highlighting/completion.
//
// The completion is **workspace-walled by construction** — it can only offer what
// the walled `Schema` contains. An empty Schema yields an empty namespace (the
// degrade contract — completion silently offers nothing, the editor still works).
// One responsibility per file (FILE-LAYOUT).

import {
  PostgreSQL,
  StandardSQL,
  type SQLConfig,
  type SQLDialect as CmSqlDialect,
  type SQLNamespace,
} from "@codemirror/lang-sql";

import type { SqlDialect } from "@/lib/panel-kit/sql/dialect";
import type { Schema } from "@/lib/schema";

/** The CodeMirror SQL dialect to use for highlighting + completion. standard → PostgreSQL (the
 *  safe superset for sqlite/postgres/timescale); surreal → StandardSQL (no SurrealQL grammar
 *  ships — StandardSQL is close enough for highlighting; a SurrealQL grammar is a deferred
 *  follow-up). */
export function toCmDialect(d: SqlDialect): CmSqlDialect {
  return d === "surreal" ? StandardSQL : PostgreSQL;
}

/** Project our Schema into @codemirror/lang-sql's completion schema (a SQLNamespace mapping each
 *  table name to its column names). An empty Schema yields an empty namespace (the degrade
 *  contract — completion silently offers nothing, the editor still works). */
export function schemaToNamespace(schema: Schema): SQLNamespace {
  const ns: Record<string, readonly string[]> = {};
  for (const t of schema.tables) ns[t.name] = t.columns.map((c) => c.name);
  return ns;
}

/** Build the SQLConfig for the editor extension. */
export function schemaConfig(dialect: SqlDialect, schema: Schema): SQLConfig {
  return {
    dialect: toCmDialect(dialect),
    schema: schemaToNamespace(schema),
    upperCaseKeywords: true,
  };
}
