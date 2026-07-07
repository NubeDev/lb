// The dialect dispatch over the SQL→model parsers (query-builder slice-1 follow-up: Code→Builder
// sync) — the inverse of `dialect.ts`'s `emitSql`. One responsibility: pick the right parser for the
// editor's dialect, plus the FROM-table salvage used when the SQL is NOT expressible (the confirm
// path keeps the raw SQL and starts the builder from the salvaged table, never from nothing when a
// table is recoverable). Both parsers return `null` on anything the model cannot express — the caller
// (SqlQueryEditor's mode switch) turns `null` into the reworded confirm.

import type { SqlBuilderQuery } from "./query";
import type { SqlDialect } from "./dialect";
import { parseStandardSql } from "./fromStandardSql";
import { parseSurrealQL } from "./fromSurrealQL";

/** Parse `sql` into the typed builder query for `dialect`, or `null` when the statement is not
 *  expressible in the builder model (subquery, CTE, window fn, multi-statement, unparseable). */
export function parseSql(dialect: SqlDialect, sql: string): SqlBuilderQuery | null {
  return dialect === "surreal" ? parseSurrealQL(sql) : parseStandardSql(sql);
}

/** Best-effort recovery of the FROM (primary) table from SQL the parsers rejected — so the confirm
 *  path can start the builder anchored on the right table instead of empty. Deliberately lenient
 *  (a plain scan, not the lexer — the SQL is unparseable by definition here): the first
 *  `FROM <quoted-or-bare identifier>`. Returns `""` when none is found (e.g. `FROM (subquery)`). */
export function salvageFromTable(sql: string): string {
  const m = /\bFROM\s+(?:"((?:[^"]|"")+)"|([A-Za-z_][A-Za-z0-9_]*))/i.exec(sql);
  if (!m) return "";
  return m[1] !== undefined ? m[1].replace(/""/g, '"') : m[2];
}
