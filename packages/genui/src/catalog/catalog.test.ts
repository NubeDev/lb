// Catalog contract: name resolution + deprecatedAliases + prompt/JSON generation goldens + the
// catalog-compat gate (genui-scope Testing plan: catalog-compat gate; catalog prompt/JSON generation
// golden files; every fixture component name resolves incl. through deprecatedAliases).
import { describe, it, expect } from "vitest";
import { nubeCatalog } from "./nubeCatalog";
import { defineCatalog } from "./defineCatalog";
import { catalogPrompt } from "./prompt";
import { toCatalogJson, catalogNames } from "./toJson";
import { IR_VERSION } from "../ir/types";

// The component names any persisted-spec fixture in the repo references. The COMPAT GATE: every one of
// these must still resolve (live or via a deprecated alias) — a component may not be removed/renamed
// without a `deprecatedAliases` entry mapping it forward.
const FIXTURE_COMPONENT_NAMES = [
  "stack", "grid", "card", "text", "markdown", "stat", "gauge", "table",
  "timeseries", "barchart", "piechart", "tag", "badge", "button", "slider", "switch",
];

describe("nubeCatalog", () => {
  it("resolves every live name", () => {
    for (const name of nubeCatalog.names()) {
      expect(nubeCatalog.resolve(name)?.name).toBe(name);
    }
  });

  it("compat gate: every fixture component name still resolves (incl. deprecated aliases)", () => {
    for (const name of FIXTURE_COMPONENT_NAMES) {
      expect(nubeCatalog.has(name)).toBe(true);
    }
    // `badge` is a deprecated alias of `tag` — resolves to the live `tag` entry.
    expect(nubeCatalog.resolve("badge")?.name).toBe("tag");
  });

  it("deprecatedAliases map forward and are NOT emitted as live names", () => {
    const cat = defineCatalog([
      { name: "new", description: "", props: {}, deprecatedAliases: ["old"], render: () => null },
    ]);
    expect(cat.resolve("old")?.name).toBe("new");
    expect(cat.names()).toEqual(["new"]); // alias excluded from the live name-set
    expect(cat.has("old")).toBe(true);
  });
});

describe("catalog generation goldens", () => {
  it("prompt block: signatures in DECLARATION order, deterministic", () => {
    const prompt = catalogPrompt(nubeCatalog);
    // Declaration order matters for positional emission — `stat(value, label, ...)` not sorted.
    expect(prompt).toContain('stat(value: binding, label?: string, unit?: string, tone?: "ok" | "warn" | "bad")');
    expect(prompt).toContain("button(label: string, value?: binding)");
    // Deterministic across runs.
    expect(catalogPrompt(nubeCatalog)).toBe(prompt);
  });

  it("catalog JSON: sorted, versioned, every component present", () => {
    const json = toCatalogJson(nubeCatalog, IR_VERSION);
    expect(json.v).toBe(IR_VERSION);
    const names = json.components.map((c) => c.name);
    expect(names).toEqual([...names].sort()); // deterministic sort
    expect(names).toContain("timeseries");
    // catalogNames = the flat live name-set the host validates against.
    expect(catalogNames(nubeCatalog)).toEqual([...nubeCatalog.names()].sort());
  });
});
