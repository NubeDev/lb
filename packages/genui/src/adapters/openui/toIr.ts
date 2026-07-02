// `elementToIr` — pure lowering of an `@openuidev/lang-core` `ElementNode` TREE into our flat,
// id-referenced IR component map (genui-scope "Flat id-referenced component map"). The lang parser nests
// via ELEMENT-VALUED PROPS (there is no `children[]` field on ElementNode): a prop whose value is another
// ElementNode — or an array of them — is a child in the render tree. So we walk props recursively,
// hoisting every nested element into its own map entry with a stable id and recording it under the
// parent's `children`, while non-element props stay as literal/binding prop values.
//
// Binding convention: the skill teaches the agent to emit data references as objects `{ $bind: "/..." }`
// (matching our IR `Binding`). Those arrive as plain object props from the parser and pass straight
// through — the adapter does not invent bindings, it only preserves the ones authored.

import type { ElementNode } from "@openuidev/lang-core";
import type { Component, IrSpec, PropValue } from "../../ir/types";
import { IR_VERSION } from "../../ir/types";
import { langNameToCatalog, langRootName } from "../../catalog/library";

function isElementNode(v: unknown): v is ElementNode {
  return typeof v === "object" && v !== null && (v as ElementNode).type === "element";
}

/** Turn a parser prop value into an IR PropValue, hoisting any nested elements into `components` and
 *  returning either the literal value or (for element props) nothing — element props become children,
 *  not prop values. Returns `{ childId }` for a single element, `{ childIds }` for an array of elements,
 *  or `{ value }` for a plain literal/binding. */
function lowerProp(
  value: unknown,
  components: Record<string, Component>,
  idOf: (node: ElementNode) => string,
): { value?: PropValue; childIds?: string[] } {
  if (isElementNode(value)) {
    return { childIds: [lowerElement(value, components, idOf)] };
  }
  if (Array.isArray(value) && value.length > 0 && value.every(isElementNode)) {
    return { childIds: value.map((el) => lowerElement(el as ElementNode, components, idOf)) };
  }
  // A mixed array or a plain array of literals is kept as a literal prop (e.g. table `columns`).
  return { value: value as PropValue };
}

/** Lower one element into the map, returning its id. Recurses through element-valued props. */
function lowerElement(
  node: ElementNode,
  components: Record<string, Component>,
  idOf: (node: ElementNode) => string,
): string {
  const id = idOf(node);
  const props: Record<string, PropValue> = {};
  const children: string[] = [];
  for (const [key, raw] of Object.entries(node.props ?? {})) {
    const { value, childIds } = lowerProp(raw, components, idOf);
    if (childIds) children.push(...childIds);
    else if (value !== undefined) props[key] = value;
  }
  components[id] = {
    id,
    component: langNameToCatalog(node.typeName),
    ...(Object.keys(props).length ? { props } : {}),
    ...(children.length ? { children } : {}),
  };
  return id;
}

/** Convert a parsed lang root `ElementNode` into a full `IrSpec`. `surfaceId` names the surface (the
 *  cell id for the dashboard tenant). A null root yields an empty spec (mid-stream / unparseable). */
export function elementToIr(root: ElementNode | null, surfaceId = "cell"): IrSpec {
  const components: Record<string, Component> = {};
  if (!root) {
    return { v: IR_VERSION, surface: { surfaceId, root: "" }, components };
  }
  // Stable ids: prefer the parser's `statementId` (the author's variable name), else a positional id.
  let counter = 0;
  const assigned = new Map<ElementNode, string>();
  const used = new Set<string>();
  const idOf = (node: ElementNode): string => {
    const existing = assigned.get(node);
    if (existing) return existing;
    let id = node.statementId && !used.has(node.statementId) ? node.statementId : `c${counter++}`;
    while (used.has(id)) id = `c${counter++}`;
    used.add(id);
    assigned.set(node, id);
    return id;
  };
  const rootId = lowerElement(root, components, idOf);
  return { v: IR_VERSION, surface: { surfaceId, root: rootId }, components };
}

/** The lang root component name the parser is configured with (re-exported for the parse/stream wiring). */
export { langRootName };
