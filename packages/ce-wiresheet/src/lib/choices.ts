// Schema-declared enum choices, e.g. a prop with `"choices": "low:0,medium:1,high:2"`.
// These come from the REST `/schema` (per component type + property) — distinct
// from per-instance `__facets` aliases. We fold them into the same `aliases` path
// so a choices prop renders as a label + edits as a dropdown everywhere, for free.

import type { Alias, PropFacet } from "./facet";

// type → propName → choices
const schemaChoices = new Map<string, Map<string, Alias[]>>();

/** Parse `"low:0,medium:1,high:2"` → aliases. `label:code` per comma-separated part. */
export function parseChoices(s: string | undefined): Alias[] | undefined {
  if (!s) return undefined;
  const out: Alias[] = [];
  for (const part of s.split(",")) {
    const t = part.trim();
    if (!t) continue;
    const j = t.lastIndexOf(":");
    if (j < 0) continue;
    const label = t.slice(0, j).trim();
    const code = Number(t.slice(j + 1).trim());
    if (label && Number.isFinite(code)) out.push({ code, label });
  }
  return out.length ? out : undefined;
}

export interface SchemaComponentDef {
  name: string;
  properties?: Array<{ name: string; choices?: string }>;
}
export interface SchemaExtDef {
  vendor: string;
  name: string;
  components?: SchemaComponentDef[];
}

/** Build the choices index from the `/schema` extension list. Full type string is
 *  `<vendor>-<ext>::<componentName>` (matches the palette). */
export function loadSchemaChoices(exts: SchemaExtDef[]): void {
  schemaChoices.clear();
  for (const e of exts) {
    const id = `${e.vendor}-${e.name}`;
    for (const c of e.components ?? []) {
      const byProp = new Map<string, Alias[]>();
      for (const p of c.properties ?? []) {
        const ch = parseChoices(p.choices);
        if (ch) byProp.set(p.name, ch);
      }
      if (byProp.size) schemaChoices.set(`${id}::${c.name}`, byProp);
    }
  }
}

export function choicesFor(type: string | undefined, propName: string): Alias[] | undefined {
  if (!type) return undefined;
  return schemaChoices.get(type)?.get(propName);
}

/** Merge schema choices into a prop facet as aliases when it has none of its own
 *  (per-instance `__facets` aliases win). */
export function withChoices(facet: PropFacet | undefined, type: string | undefined, propName: string): PropFacet | undefined {
  if (facet?.aliases?.length) return facet;
  const ch = choicesFor(type, propName);
  if (!ch) return facet;
  return { ...(facet ?? {}), aliases: ch };
}
