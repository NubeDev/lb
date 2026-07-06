// Unit tests for the ported sqlSplitter + our SqlDialect→Dialect adapter.
// Pure-TS, fixture-based (rule 9 — no mocks). Covers the cases the
// query-builder 10x scope pins: ;-split, dollar-quoting, DELIMITER,
// comment-folding, classification, dialect mapping, empty input.

import { describe, expect, it } from "vitest";

import type { SqlDialect } from "@/lib/panel-kit/sql/dialect";

import {
  isSelect,
  returnsResultSet,
  splitStatements,
} from "./index";
import { toSplitterDialect } from "./fromSqlDialect";

describe("splitStatements", () => {
  // NOTE: the ported splitter builds `text` from the meaningful spans (excluding the trailing
  // delimiter token), so the trailing `;` / `$$` is NOT part of `stmt.text`. This matches the
  // verbatim port — do not "fix" it.
  it("splits two ;-separated SELECTs into 2 statements, each isSelect", () => {
    const sql = "SELECT 1;\nSELECT 2;";
    const stmts = splitStatements(sql, "postgres");
    expect(stmts).toHaveLength(2);
    expect(stmts[0].text).toBe("SELECT 1");
    expect(stmts[1].text).toBe("SELECT 2");
    expect(stmts.every((s) => s.isSelect)).toBe(true);
  });

  it("does not split inside a $$ ... $$ dollar-quoted block (postgres)", () => {
    const body = "SELECT $$\nhello;\nworld\n$$;\nSELECT 2;";
    const stmts = splitStatements(body, "postgres");
    expect(stmts).toHaveLength(2);
    expect(stmts[0].text).toContain("$$");
    expect(stmts[0].text).toContain("hello;");
    expect(stmts[0].isSelect).toBe(true);
    expect(stmts[1].text).toBe("SELECT 2");
  });

  it("honours a MySQL DELIMITER directive (custom delimiter)", () => {
    const sql = "DELIMITER $$\nSELECT 1$$\nSELECT 2$$";
    const stmts = splitStatements(sql, "mysql");
    expect(stmts).toHaveLength(2);
    expect(stmts[0].text).toBe("SELECT 1");
    expect(stmts[1].text).toBe("SELECT 2");
    expect(stmts.every((s) => s.isSelect)).toBe(true);
  });

  it("folds a leading comment-only fragment into the next statement", () => {
    // For a comment to be its own segment it must be delimiter-bounded; the delimiter after
    // the comment is what produces a `hasMeaningful:false` segment that then folds into the
    // following SELECT. (A bare `-- c\nSELECT 1` keeps the comment inside the meaningful span.)
    const sql = "-- a comment\n;\nSELECT 1;";
    const stmts = splitStatements(sql, "postgres");
    expect(stmts).toHaveLength(1);
    // The text includes the folded comment (the trailing ; is the delimiter, not in text).
    expect(stmts[0].text).toContain("-- a comment");
    expect(stmts[0].text).toContain("SELECT 1");
    // The meaningful range starts at "SELECT", not at the comment.
    expect(stmts[0].range.start).toBe(sql.indexOf("SELECT"));
  });

  it("returns [] for empty input", () => {
    expect(splitStatements("", "postgres")).toEqual([]);
  });
});

describe("isSelect", () => {
  // NOTE: the verbatim port's `isSelect` matches the leading keyword whole-word against `SELECT`.
  // A `WITH ...` statement therefore classifies as NOT a SELECT here — it is a result-set statement
  // (covered by `returnsResultSet` below). Do not "fix" this in the port; the original behaviour
  // is intentional.
  it("classifies a SELECT as select; INSERT and WITH as not", () => {
    expect(isSelect("SELECT 1")).toBe(true);
    expect(isSelect("WITH x AS (SELECT 1) SELECT * FROM x")).toBe(false);
    expect(isSelect("INSERT INTO t VALUES (1)")).toBe(false);
  });

  it("does not misclassify SELECTIVE as SELECT (whole-word match)", () => {
    expect(isSelect("SELECTIVE * FROM t")).toBe(false);
  });
});

describe("returnsResultSet", () => {
  it.each([
    ["SELECT 1", true],
    ["WITH x AS (SELECT 1) SELECT * FROM x", true],
    ["SHOW TABLES", true],
    ["VALUES (1), (2)", true],
    ["INSERT INTO t VALUES (1)", false],
    ["UPDATE t SET x = 1", false],
  ])("%s → %s", (sql, expected) => {
    expect(returnsResultSet(sql)).toBe(expected);
  });
});

describe("toSplitterDialect", () => {
  it("maps surreal → generic and standard → postgres", () => {
    expect(toSplitterDialect("surreal" as SqlDialect)).toBe("generic");
    expect(toSplitterDialect("standard" as SqlDialect)).toBe("postgres");
  });
});
