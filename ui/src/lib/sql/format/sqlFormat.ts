// Adapted from Tabularis (github.com/TabularisDB/tabularis), Apache-2.0.
//
// The SQL formatter wrapper (query-builder 10x, slice 2). One responsibility: beautify a SQL
// string via `sql-formatter`, mapped onto OUR `SqlDialect` (standard → postgresql; surreal →
// `sql` fallback, which is why the Format button is GATED to standard dialect in SqlQueryHeader —
// sql-formatter has no SurrealQL grammar and its `sql` fallback can mangle `table:id`, `type::`,
// `->`). Returns the input unchanged on empty or syntactically-incomplete input so a caller never
// loses the user's text. Pure TS, no React.

import {
  format,
  type FormatOptionsWithLanguage,
  type SqlLanguage,
} from "sql-formatter";

import type { SqlDialect } from "@/lib/panel-kit/sql/dialect";

/** Map our SqlDialect to sql-formatter's language. standard → postgresql (the safe superset);
 *  surreal → falls back to standard "sql" (sql-formatter has NO SurrealQL grammar — its `sql`
 *  fallback can corrupt Surreal syntax like `table:id`, `type::`, `->`; for that reason the
 *  Format button is GATED to standard dialect in SqlQueryHeader). */
export function toFormatterLanguage(d: SqlDialect | undefined): SqlLanguage {
  return d === "surreal" ? "sql" : "postgresql";
}

/** Beautify a SQL string for the given dialect. Returns the input unchanged when it is empty or
 *  when the formatter throws (syntactically incomplete SQL), so callers never lose the user's text. */
export function formatSql(
  sql: string,
  dialect?: SqlDialect,
  options?: Partial<FormatOptionsWithLanguage>,
): string {
  if (!sql.trim()) return sql;
  try {
    return format(sql, {
      language: toFormatterLanguage(dialect),
      keywordCase: "upper",
      ...options,
    });
  } catch {
    return sql;
  }
}
