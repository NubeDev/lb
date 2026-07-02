// `toCatalogJson` — the A2UI-style catalog JSON generated from `defineCatalog`. This is the artifact the
// Rust host embeds and validates a saved genui cell against (genui-scope Decision 6): every `component`
// name in a spec must appear in `components[]` here. It is ALSO the stable name-set the CI drift gate
// pins. Deterministic (sorted) so a byte-identity check is meaningful.

import type { Catalog, PropSpec } from "./defineCatalog";

export interface CatalogJsonProp {
  type: PropSpec["type"];
  description?: string;
  required?: boolean;
  values?: string[];
}

export interface CatalogJsonComponent {
  name: string;
  description: string;
  props: Record<string, CatalogJsonProp>;
  actions?: string[];
  deprecatedAliases?: string[];
}

export interface CatalogJson {
  /** IR schema version this catalog targets — the host cross-checks a spec's `v` against known ones. */
  v: number;
  components: CatalogJsonComponent[];
}

export function toCatalogJson(catalog: Catalog, v: number): CatalogJson {
  const components = [...catalog.entries]
    .sort((a, b) => a.name.localeCompare(b.name))
    .map((e): CatalogJsonComponent => {
      const props: Record<string, CatalogJsonProp> = {};
      for (const key of Object.keys(e.props).sort()) {
        const p = e.props[key];
        props[key] = { type: p.type };
        if (p.description) props[key].description = p.description;
        if (p.required) props[key].required = true;
        if (p.values) props[key].values = p.values;
      }
      const out: CatalogJsonComponent = { name: e.name, description: e.description, props };
      if (e.actions?.length) out.actions = [...e.actions];
      if (e.deprecatedAliases?.length) out.deprecatedAliases = [...e.deprecatedAliases];
      return out;
    });
  return { v, components };
}

/** The flat name-set (live names only) — what the host membership-checks against. Sorted, stable. */
export function catalogNames(catalog: Catalog): string[] {
  return [...catalog.names()].sort();
}
