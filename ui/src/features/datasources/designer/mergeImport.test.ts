// Unit tests for the import merge (schema-designer scope). Pure: a discovered catalog → a designer
// record with deduped tables, seeded grid layout, and inferred FKs. The fixture mirrors the
// demo-buildings (sqlite) datasource the designer imports from — site / meter / point / point_reading
// + the *_tag / meter_tag join tables — with `<table>_id` ref columns, so the inferred FKs are the
// same relationship lines the read-only Discovery → Diagram shows for that source.

import { describe, expect, it } from "vitest";

import { mergeImport, guessNeutralType, type DiscoveredTable } from "./mergeImport";
import type { DbSchemaRecord } from "@/lib/datasources";

/** A discovered table with minimal column noise (type only matters for the neutral-type mapping). */
function disc(name: string, cols: [string, string][]): DiscoveredTable {
  return { name, columns: cols.map(([n, dataType]) => ({ name: n, dataType, nullable: true })) };
}

// The demo-buildings shape (from the datasource's Diagram): site is a root; meter refs site via
// `site_id`; point refs meter via `meter_id`; the reading + *_tag tables ref their parent via
// `<parent>_id`.
const DEMO: DiscoveredTable[] = [
  disc("site", [["id", "text"], ["name", "text"]]),
  disc("meter", [["id", "text"], ["site_id", "text"], ["name", "text"]]),
  disc("point", [["id", "text"], ["meter_id", "text"], ["name", "text"]]),
  disc("point_reading", [["time", "text"], ["point_id", "text"], ["value", "real"]]),
  disc("point_tag", [["point_id", "text"], ["tag", "text"], ["kind", "text"], ["val", "text"]]),
  disc("meter_tag", [["meter_id", "text"], ["tag", "text"], ["kind", "text"], ["val", "text"]]),
  disc("site_tag", [["site_id", "text"], ["tag", "text"], ["kind", "text"], ["val", "text"]]),
];

const EMPTY: DbSchemaRecord = { name: "shop", version: 1, tables: [], fks: [], layout: {} };

describe("mergeImport — tables", () => {
  it("appends every discovered table (mapping catalog types to neutral types)", () => {
    const r = mergeImport(EMPTY, DEMO);
    expect(r.tables.map((t) => t.name).sort()).toEqual(
      ["meter", "meter_tag", "point", "point_reading", "point_tag", "site", "site_tag"],
    );
    const value = r.tables.find((t) => t.name === "point_reading")!.columns.find((c) => c.name === "value")!;
    expect(value.type).toBe("real");
  });

  it("does not duplicate a table already in the record (dedup by name)", () => {
    const seeded: DbSchemaRecord = { ...EMPTY, tables: [{ name: "site", pk: [], columns: [] }] };
    const r = mergeImport(seeded, DEMO);
    expect(r.tables.filter((t) => t.name === "site")).toHaveLength(1);
  });

  it("seeds a distinct layout position for each imported table (no {0,0} stacking)", () => {
    const r = mergeImport(EMPTY, DEMO);
    const positions = Object.values(r.layout).map((p) => `${p.x},${p.y}`);
    expect(new Set(positions).size).toBe(DEMO.length); // all distinct
    expect(positions).not.toContain("0,0");
  });
});

describe("mergeImport — inferred FKs (the relationship lines)", () => {
  it("infers a declared FK per `<table>_id` ref column, targeting the parent's id", () => {
    const r = mergeImport(EMPTY, DEMO);
    const keys = r.fks.map((f) => `${f.fromTable}.${f.fromColumns[0]}->${f.toTable}.${f.toColumns[0]}`).sort();
    expect(keys).toEqual([
      "meter.site_id->site.id",
      "meter_tag.meter_id->meter.id",
      "point.meter_id->meter.id",
      "point_reading.point_id->point.id",
      "point_tag.point_id->point.id",
      "site_tag.site_id->site.id",
    ]);
  });

  it("does not re-add an FK the record already declares (dedup against existing)", () => {
    const seeded: DbSchemaRecord = {
      ...EMPTY,
      fks: [{ name: "", fromTable: "meter", fromColumns: ["site_id"], toTable: "site", toColumns: ["id"] }],
    };
    const r = mergeImport(seeded, DEMO);
    expect(r.fks.filter((f) => f.fromTable === "meter" && f.fromColumns[0] === "site_id")).toHaveLength(1);
  });
});

describe("guessNeutralType", () => {
  it("maps common catalog type names to the neutral vocabulary", () => {
    expect(guessNeutralType("INTEGER")).toBe("integer");
    expect(guessNeutralType("VARCHAR(255)")).toBe("text");
    expect(guessNeutralType("DOUBLE PRECISION")).toBe("real");
    expect(guessNeutralType("BOOLEAN")).toBe("boolean");
    expect(guessNeutralType("TIMESTAMPTZ")).toBe("timestamp");
    expect(guessNeutralType("something-unknown")).toBe("text");
  });
});
