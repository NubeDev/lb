// Unit tests for the Rhai re-indenter (rules-editor-ux). The load-bearing property: a wall-of-text
// rule body (many statements on one line, brace-heavy) comes out as one statement per line with
// nesting-depth indentation, and strings/comments are never mistaken for structural `;`/`{`/`}`.

import { describe, expect, it } from "vitest";

import { formatRhai } from "./formatRhai";

describe("formatRhai", () => {
  it("splits a single-line rule into indented statements", () => {
    const source =
      'let rows = query("timeseries", "SELECT 1").records(); let peak = rows[0][0]; let sev = "info"; if peak > 15.0 { sev = "critical"; } emit(#{ metric: "x", peak_kw: peak });';

    const out = formatRhai(source);

    expect(out).toBe(
      [
        'let rows = query("timeseries", "SELECT 1").records();',
        "let peak = rows[0][0];",
        'let sev = "info";',
        "if peak > 15.0 {",
        '  sev = "critical";',
        "}",
        'emit(#{ metric: "x", peak_kw: peak });',
        "",
      ].join("\n"),
    );
  });

  it("does not treat semicolons or braces inside strings as structural", () => {
    const source = 'let title = "a; b { c }"; let n = 1;';

    const out = formatRhai(source);

    expect(out).toBe(['let title = "a; b { c }";', "let n = 1;", ""].join("\n"));
  });

  it("keeps nested blocks indented one level deeper per brace", () => {
    const source = "if a { if b { c(); } }";

    const out = formatRhai(source);

    expect(out).toBe(["if a {", "  if b {", "    c();", "  }", "}", ""].join("\n"));
  });

  it("is idempotent on already-formatted input", () => {
    const once = formatRhai('let x = 1; if x > 0 { emit(#{ a: 1 }); }');
    const twice = formatRhai(once);
    expect(twice).toBe(once);
  });

  it("keeps nested #{ } object-map literals on one line as a statement, not a block", () => {
    const source =
      'insight.raise(#{ severity: sev, body: #{ peak_kw: peak }, origin: #{ kind: "rule" } });';

    const out = formatRhai(source);

    expect(out).toBe(source + "\n");
  });

  it("wraps a long SQL literal inside query(...) as a backtick multiline string", () => {
    const source =
      'query("timescale", "SELECT a, b FROM t JOIN u ON t.id = u.id WHERE a > 1 GROUP BY b ORDER BY a")';

    const out = formatRhai(source);

    expect(out).toContain("query(\"timescale\", `\n");
    expect(out).toContain("SELECT");
    expect(out).toContain("\n`)");
    // The value is preserved (SQL is whitespace-insensitive) — every column/table survives.
    expect(out).toContain("FROM");
    expect(out).toContain("GROUP BY");
  });

  it("is idempotent on a body whose query() SQL was already wrapped", () => {
    const once = formatRhai(
      'let rows = query("ts", "SELECT a FROM t WHERE a = 1 GROUP BY a ORDER BY a").records();',
    );
    expect(formatRhai(once)).toBe(once);
  });
});
