// `defineCatalog` — the ONLY thing the agent may instantiate (genui-scope "src/catalog/"). The
// constraint is STRUCTURAL: a component name not in the catalog cannot render (normalize turns it into
// a placeholder at authoring time; the host rejects it at save time). One catalog drives BOTH prompt
// surfaces (component-signature block + A2UI-style catalog JSON) and the React render fns.
//
// The compat/drift rule (genui-scope "Versioning & catalog compatibility"): a component or prop may not
// be removed/renamed without a `deprecatedAliases` entry mapping the old name forward — `resolveName`
// honours it so a persisted spec degrades only when its GRANT changes, never because the catalog was
// refactored.

import type { ReactNode } from "react";

/** A JSON-Schema-ish prop descriptor. Kept deliberately small — enough to generate a signature line and
 *  to type-coerce in normalize; not a full validator. `type` drives the coercion in normalize. */
export interface PropSpec {
  type: "string" | "number" | "boolean" | "array" | "object" | "enum" | "binding";
  description?: string;
  required?: boolean;
  /** For `type:"enum"`. */
  values?: string[];
  default?: unknown;
}

/** The props a catalog entry's render fn receives: resolved (bindings already swapped) props, the
 *  resolved children (already-rendered React nodes, in `children` order), and the action dispatch. */
export interface RenderProps {
  props: Record<string, unknown>;
  children: ReactNode[];
  /** Emit a control action over the leashed bridge. `name` is the entry's action; `context` the args. */
  emit: (name: string, context?: Record<string, unknown>) => void;
}

export interface CatalogEntry {
  name: string;
  description: string;
  props: Record<string, PropSpec>;
  /** Catalog action names this component can `emit` (button/slider/switch); used by the prompt. */
  actions?: string[];
  render: (rp: RenderProps) => ReactNode;
  /** Old names/prop-names that map forward to THIS entry (the drift rule). */
  deprecatedAliases?: string[];
}

export interface Catalog {
  entries: CatalogEntry[];
  /** Resolve a (possibly deprecated) component name to a live entry, or `undefined`. */
  resolve: (name: string) => CatalogEntry | undefined;
  /** The set of every LIVE component name (not aliases) — what `toJson`/the host validate against. */
  names: () => string[];
  /** True if `name` resolves (live or via a deprecated alias). */
  has: (name: string) => boolean;
}

export function defineCatalog(entries: CatalogEntry[]): Catalog {
  const byName = new Map<string, CatalogEntry>();
  for (const e of entries) {
    byName.set(e.name, e);
    for (const alias of e.deprecatedAliases ?? []) byName.set(alias, e);
  }
  const resolve = (name: string) => byName.get(name);
  return {
    entries,
    resolve,
    names: () => entries.map((e) => e.name),
    has: (name: string) => byName.has(name),
  };
}
