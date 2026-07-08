// formatSqlLiterals — reformat the SQL string argument inside a Rhai `query(source, "SELECT …")`
// call so a wall-of-text query wraps readably (rules-editor-ux scope). Rhai's `query()` runs the
// second argument as SQL against a federated source; that string is what grows long. This finds a
// `query( <src>, <sql-string> )` shape, beautifies the SQL via the shared `formatSql`, and re-emits
// it as a Rhai backtick raw string (`…`) — backticks are the ONE Rhai literal that may span lines,
// so the multi-line SQL parses. SQL is whitespace-insensitive, so the runtime value is equivalent.
// Only `query(` is touched — `history()`/`ai.complete()` args carry no SQL. One responsibility.

import { formatSql } from "@/lib/sql/format/sqlFormat";

/** Below this SQL length, wrapping onto many lines is more noise than help — leave it inline. */
const WRAP_MIN = 60;

/** Reformat every `query(src, "<sql>")` SQL literal on one statement `line`, re-indented under
 *  `baseIndent`. Returns the line unchanged when it holds no such call. `formatSql` returns its input
 *  unchanged on unparseable SQL, so a malformed query is never corrupted. */
export function formatSqlLiterals(line: string, baseIndent: string): string {
  if (!line.includes("query(")) return line;

  // Match `query(` then a source argument (any non-comma run — usually a "…" literal), a comma, then
  // the SQL string literal (single OR double quoted, honouring `\`-escapes). The SQL group is what we
  // reformat; everything else is preserved verbatim.
  const CALL = /query\(\s*([^,]+?)\s*,\s*(["'])((?:\\.|(?!\2).)*)\2\s*\)/g;

  return line.replace(CALL, (whole, src: string, _q: string, sql: string) => {
    const raw = sql.replace(/\\(["'\\])/g, "$1"); // un-escape so the formatter sees real SQL
    if (raw.includes("`")) return whole; // can't safely re-wrap into a backtick literal — leave as-is
    if (raw.length < WRAP_MIN) return whole; // short queries read fine inline — don't add noise

    const pretty = formatSql(raw, "standard");
    if (!pretty.includes("\n")) return whole; // already short / formatter made no change

    // Indent each SQL line one level under the statement, close the backtick on its own line.
    const inner = pretty
      .split("\n")
      .map((l) => baseIndent + "  " + l)
      .join("\n");
    return `query(${src.trim()}, \`\n${inner}\n${baseIndent}\`)`;
  });
}
