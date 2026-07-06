// Unit tests for the SQL formatter wrapper (slice 2). Pure-TS, no mocks (rule 9).
// Pins: pretty-prints + uppercases; idempotent; preserves empty input; preserves
// syntactically-incomplete input unchanged; dialect→language mapping.

import { describe, expect, it } from "vitest";

import type { SqlDialect } from "@/lib/panel-kit/sql/dialect";

import { formatSql, toFormatterLanguage } from "./sqlFormat";

describe("formatSql", () => {
  it("pretty-prints + uppercases a messy SELECT", () => {
    const out = formatSql("select a,  b from t where c='x'", "standard");
    expect(out).toContain("SELECT");
    expect(out).toContain("FROM");
    expect(out).toContain("WHERE");
    // Multiple spaces collapsed; the original had "a,  b".
    expect(out).not.toContain("a,  b");
    expect(out).toContain("a,");
    expect(out).toContain("'x'");
  });

  it("is idempotent (format ∘ format === format)", () => {
    const input = "select a,  b from t where c='x'";
    const once = formatSql(input, "standard");
    expect(formatSql(once, "standard")).toBe(once);
  });

  it.each(["", "   ", "\n\t"])("returns empty/whitespace input unchanged: %r", (s) => {
    expect(formatSql(s, "standard")).toBe(s);
  });

  it("returns syntactically-broken SQL unchanged (no throw)", () => {
    // sql-formatter is forgiving of incomplete SQL (e.g. `SELECT FROM` parses), so use a token
    // its grammar rejects outright — an unterminated string literal — to exercise the catch path.
    const broken = "'unclosed string";
    expect(formatSql(broken, "standard")).toBe(broken);
  });
});

describe("toFormatterLanguage", () => {
  it("maps surreal → 'sql' and standard → 'postgresql'", () => {
    expect(toFormatterLanguage("surreal" as SqlDialect)).toBe("sql");
    expect(toFormatterLanguage("standard" as SqlDialect)).toBe("postgresql");
  });

  it("maps undefined → postgresql (the safe default)", () => {
    expect(toFormatterLanguage(undefined)).toBe("postgresql");
  });
});
