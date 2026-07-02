// `buildLangLibrary` — build the OpenUI-Lang parser schema (`LibraryJSONSchema`) BY HAND from our
// catalog, rather than wiring real zod schemas per component. This is deliberate: `@openuidev/lang-core`
// types its schemas with `zod/v4/core`, and hand-building the `{$defs: {Comp: {properties, required}}}`
// object is both simpler and immune to a zod major-version mismatch. `createStreamingParser` /
// `createParser` accept a plain `LibraryJSONSchema` directly (see their signatures in the d.mts), so no
// zod instance is ever needed here.
//
// The parser uses each def's `properties` KEY ORDER for positional-arg → named-prop mapping (the
// `Header("Hello", "Subtitle")` convention) and `required` to drop under-specified components. We emit
// properties in the catalog's declared prop order so positional args line up with author intent.
//
// ── Name mapping decision ─────────────────────────────────────────────────────────────────────────
// OpenUI Lang is PascalCase (`Stat(...)`, `BarChart(...)`); our catalog names are lowercase (`stat`,
// `barchart`). We map catalog → lang name by PascalCasing, with an EXPLICIT override table for the
// multi-word names so `barchart` → `BarChart` (not `Barchart`) etc. The adapter that turns a parsed
// lang tree back into IR component names uses `langNameToCatalog` to translate in reverse. Deprecated
// aliases are NOT emitted as lang components (the agent authors only live names); the drift rule stays a
// catalog concern.

import type { LibraryJSONSchema } from "@openuidev/lang-core";
import type { Catalog, CatalogEntry, PropSpec } from "./defineCatalog";

/** The root STATEMENT id OpenUI Lang uses as the entry point. Lang is statement-based
 *  (`id = Component(...)`, one per line) and the parser's entry point is the statement named `root`
 *  (`DEFAULT_ROOT_STATEMENT_ID` in lang-core) — this is a statement id, NOT a component name, so no
 *  synthetic root component is needed. `createParser(schema, langRootName())` pins it explicitly. */
export function langRootName(): string {
  return "root";
}

// Explicit catalog→lang overrides for names that don't PascalCase cleanly (multi-word, run-together).
const LANG_NAME_OVERRIDES: Record<string, string> = {
  barchart: "BarChart",
  piechart: "PieChart",
  timeseries: "TimeSeries",
};

/** catalog name (`stat`, `barchart`) → lang name (`Stat`, `BarChart`). */
export function catalogToLangName(name: string): string {
  if (LANG_NAME_OVERRIDES[name]) return LANG_NAME_OVERRIDES[name];
  return name.length === 0 ? name : name[0].toUpperCase() + name.slice(1);
}

/** lang name (`Stat`, `BarChart`) → catalog name (`stat`, `barchart`). Built from the same table so the
 *  two directions can never drift. Falls back to lowercasing the first char. */
export function langNameToCatalog(langName: string): string {
  for (const [cat, lang] of Object.entries(LANG_NAME_OVERRIDES)) {
    if (lang === langName) return cat;
  }
  return langName.length === 0 ? langName : langName[0].toLowerCase() + langName.slice(1);
}

/** Map one PropSpec to a JSON-Schema-ish property node the parser can carry (type + default). The parser
 *  only needs enough to know the param exists and its default; we keep it faithful but minimal. */
function propToSchema(spec: PropSpec): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  switch (spec.type) {
    case "enum":
      out.type = "string";
      if (spec.values) out.enum = spec.values;
      break;
    case "binding":
      // A binding resolves to any JSON value at render time; leave the type open.
      break;
    case "string":
    case "number":
    case "boolean":
    case "array":
    case "object":
      out.type = spec.type;
      break;
  }
  if (spec.description) out.description = spec.description;
  if (spec.default !== undefined) out.default = spec.default;
  return out;
}

function entryToDef(entry: CatalogEntry): { properties: Record<string, unknown>; required: string[] } {
  const properties: Record<string, unknown> = {};
  const required: string[] = [];
  // Preserve declared order for positional-arg mapping.
  for (const [propName, spec] of Object.entries(entry.props)) {
    properties[propName] = propToSchema(spec);
    if (spec.required) required.push(propName);
  }
  return { properties, required };
}

/** Build the parser schema from the catalog. One `$defs` entry per LIVE catalog component (keyed by its
 *  lang PascalCase name), plus a synthetic `Surface` root that accepts arbitrary children. */
export function buildLangLibrary(catalog: Catalog): LibraryJSONSchema {
  const $defs: NonNullable<LibraryJSONSchema["$defs"]> = {};

  for (const entry of catalog.entries) {
    $defs[catalogToLangName(entry.name)] = entryToDef(entry);
  }

  return { $defs };
}
