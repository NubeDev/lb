// Resolve the singleton alarm service (role "alarm.service") globally via REST.
// It lives in the services folder; the structural store only holds the current
// folder, so we walk the tree and match by type (robust to folder name/nesting).
// Shared by the live console (AlarmPanel) and the history view.

import { getRootNodes } from "../lib/rest";
import type { Component } from "../lib/engine-types";

const lastSeg = (t: string) => t.toLowerCase().split(/[:/.\\]+/).filter(Boolean).pop() ?? t.toLowerCase();

function find(nodes: Component[]): Component | undefined {
  for (const c of nodes) {
    if (lastSeg(c.type) === "alarm") return c;
    const inChild = c.children ? find(c.children) : undefined;
    if (inChild) return inChild;
  }
  return undefined;
}

export async function resolveAlarmService(): Promise<Component | undefined> {
  const r = await getRootNodes({ depth: 3, nested: true });
  return find(r.nodes);
}
