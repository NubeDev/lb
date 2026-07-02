// Resolve the singleton JS script store (manifest: role "js.service", type
// "…::jsScriptStore") globally via REST. Like the alarm service it lives in the
// services folder, while the structural store only holds the current folder —
// so walk the tree and match by the type's last segment.

import { getRootNodes } from "../lib/rest";
import type { Component } from "../lib/engine-types";

const lastSeg = (t: string) => t.toLowerCase().split(/[:/.\\]+/).filter(Boolean).pop() ?? t.toLowerCase();

function find(nodes: Component[], seg: string): Component | undefined {
  for (const c of nodes) {
    if (lastSeg(c.type) === seg) return c;
    const inChild = c.children ? find(c.children, seg) : undefined;
    if (inChild) return inChild;
  }
  return undefined;
}

/** The jsScriptStore singleton, or undefined if it isn't on this engine yet. */
export async function resolveJsStore(typeSeg = "jsscriptstore"): Promise<Component | undefined> {
  const r = await getRootNodes({ depth: 3, nested: true });
  return find(r.nodes, typeSeg.toLowerCase());
}
