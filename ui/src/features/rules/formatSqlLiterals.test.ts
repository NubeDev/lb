// Unit tests for the query(...) SQL-literal reformatter (rules-editor-ux). Load-bearing properties:
// only `query()` SQL args are touched, the SQL VALUE is preserved (whitespace-insensitive), a
// backtick already present is left alone (can't safely re-wrap), and a non-query line is untouched.

import { describe, expect, it } from "vitest";

import { formatSqlLiterals } from "./formatSqlLiterals";

describe("formatSqlLiterals", () => {
  it("rewrites a query() double-quoted SQL literal into a backtick raw string", () => {
    const line =
      'query("ts", "SELECT a, b, c FROM some_table WHERE a > 1 GROUP BY b ORDER BY a")';

    const out = formatSqlLiterals(line, "");

    expect(out.startsWith("query(\"ts\", `\n")).toBe(true);
    expect(out.trimEnd().endsWith("`)")).toBe(true);
    expect(out).toContain("SELECT");
    expect(out).toContain("GROUP BY");
  });

  it("indents the wrapped SQL under the given base indent", () => {
    const line = 'query("ts", "SELECT a, b, c FROM some_table WHERE a > 1 GROUP BY a ORDER BY a")';

    const out = formatSqlLiterals(line, "  ");

    // Each SQL line sits one level (2 spaces) deeper than the 2-space base.
    expect(out).toContain("\n    SELECT");
    expect(out.trimEnd().endsWith("\n  `)")).toBe(true);
  });

  it("leaves a line with no query() call untouched", () => {
    const line = 'emit(#{ msg: "hello; SELECT everywhere { }" });';
    expect(formatSqlLiterals(line, "")).toBe(line);
  });

  it("leaves a query() whose SQL already contains a backtick untouched", () => {
    const line = 'query("ts", "SELECT `weird col` FROM t")';
    expect(formatSqlLiterals(line, "")).toBe(line);
  });

  it("does not touch history()/other calls that carry no SQL", () => {
    const line = 'history("series", "cooler.temp", "24h")';
    expect(formatSqlLiterals(line, "")).toBe(line);
  });
});
