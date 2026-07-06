// Unit tests for the Schema→@codemirror/lang-sql completion projection (slice 2).
// Pure-TS, no mocks (rule 9). Pins the SQLConfig shape against the installed
// @codemirror/lang-sql so a version bump cannot silently break completion.

import { PostgreSQL, StandardSQL } from "@codemirror/lang-sql";
import { describe, expect, it } from "vitest";

import type { Schema } from "@/lib/schema";

import { schemaConfig, schemaToNamespace, toCmDialect } from "./sqlCompletion";

const twoTable: Schema = {
  tables: [
    { name: "t1", columns: [{ name: "a", type: "int" }, { name: "b", type: "int" }] },
    { name: "t2", columns: [{ name: "x", type: "int" }] },
  ],
};

describe("schemaToNamespace", () => {
  it("projects each table to its column-name list", () => {
    expect(schemaToNamespace(twoTable)).toEqual({
      t1: ["a", "b"],
      t2: ["x"],
    });
  });

  it("returns {} for an empty schema (the degrade contract)", () => {
    expect(schemaToNamespace({ tables: [] })).toEqual({});
  });
});

describe("toCmDialect", () => {
  it("maps surreal → StandardSQL and standard → PostgreSQL", () => {
    expect(toCmDialect("surreal")).toBe(StandardSQL);
    expect(toCmDialect("standard")).toBe(PostgreSQL);
  });
});

describe("schemaConfig", () => {
  it("includes the schema namespace, dialect, and upperCaseKeywords", () => {
    const cfg = schemaConfig("standard", twoTable);
    expect(cfg.upperCaseKeywords).toBe(true);
    expect(cfg.dialect).toBe(PostgreSQL);
    expect(cfg.schema).toEqual({ t1: ["a", "b"], t2: ["x"] });
  });

  it("yields an empty namespace for an empty schema", () => {
    const cfg = schemaConfig("surreal", { tables: [] });
    expect(cfg.schema).toEqual({});
    expect(cfg.dialect).toBe(StandardSQL);
  });
});
