// `catalogPrompt` — the OpenUI-style component-signature block generated from `defineCatalog`. This is
// the text the `genui-widget` skill embeds (via `pnpm --filter @nube/genui gen:skill`) to teach the
// agent EXACTLY the components it may emit and their props. Generated, never hand-written, so the skill
// can never lag the catalog (genui-scope "The codegen chain is named"). Deterministic (sorted).

import type { Catalog, CatalogEntry, PropSpec } from "./defineCatalog";

function propSig(name: string, p: PropSpec): string {
  const opt = p.required ? "" : "?";
  let ty: string = p.type;
  if (p.type === "enum" && p.values) ty = p.values.map((v) => JSON.stringify(v)).join(" | ");
  if (p.type === "binding") ty = "binding";
  return `${name}${opt}: ${ty}`;
}

function entrySig(e: CatalogEntry): string {
  const props = Object.keys(e.props)
    .sort()
    .map((k) => propSig(k, e.props[k]))
    .join(", ");
  const actions = e.actions?.length ? `  actions: ${e.actions.join(", ")}\n` : "";
  return `- ${e.name}(${props})\n    ${e.description}\n${actions}`;
}

export function catalogPrompt(catalog: Catalog): string {
  const lines = [...catalog.entries].sort((a, b) => a.name.localeCompare(b.name)).map(entrySig);
  return ["Components you may use (and ONLY these):", "", ...lines].join("\n").trimEnd() + "\n";
}
