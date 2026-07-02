// `parseLang` — the one-shot OpenUI-Lang → IR authoring adapter (genui-scope "src/adapters/openui/").
// Parses a complete Lang document via `@openuidev/lang-core`'s `createParser` (built from OUR catalog's
// hand-authored `LibraryJSONSchema`), then lowers the parsed `ElementNode` tree to our flat IR via
// `elementToIr`. Authoring-stratum only — never on the render path. The parser's own `meta` (unresolved
// refs, orphans, incomplete) is surfaced as warnings so the author sees emission sloppiness before
// accept; `normalize` (a separate pass) then fixes catalog-level sloppiness.

import { createParser } from "@openuidev/lang-core";
import type { Catalog } from "../../catalog/defineCatalog";
import { buildLangLibrary, langRootName } from "../../catalog/library";
import type { Finding, IrSpec } from "../../ir/types";
import { elementToIr } from "./toIr";

export interface ParseResultIr {
  ir: IrSpec;
  findings: Finding[];
}

function metaFindings(meta: { incomplete?: boolean; unresolved?: string[]; orphaned?: string[] }): Finding[] {
  const findings: Finding[] = [];
  if (meta.incomplete) {
    findings.push({ level: "warning", code: "incomplete-emission", message: "emission looks truncated" });
  }
  for (const u of meta.unresolved ?? []) {
    findings.push({ level: "warning", code: "unresolved-ref", message: `reference "${u}" was never defined (dropped)` });
  }
  for (const o of meta.orphaned ?? []) {
    findings.push({ level: "warning", code: "orphaned-statement", message: `statement "${o}" is unreachable from the root` });
  }
  return findings;
}

export function parseLang(text: string, catalog: Catalog, surfaceId = "cell"): ParseResultIr {
  const parser = createParser(buildLangLibrary(catalog), langRootName());
  const result = parser.parse(text);
  return { ir: elementToIr(result.root, surfaceId), findings: metaFindings(result.meta) };
}
