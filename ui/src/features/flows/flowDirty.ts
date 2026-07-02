// The dirty comparator (flow-deploy-ux scope) — the single source of "does the canvas differ from the
// deployed flow?". Node-RED posture: the canvas is a draft; nothing reaches the running system until
// Deploy, and Deploy is enabled ONLY when this returns true. One pure function so the toolbar, the hook,
// and tests agree (FILE-LAYOUT: the named concept, not a util dump).
//
// We compare the GRAPH SHAPE the author edits — each node's `type`, its `needs` (the wiring), its
// `config`, and its `with` bindings — normalized so serialization noise (key order, needs order) never
// reads as a change. We do NOT compare `version` (Deploy bumps it), `workspace`/lifecycle flags
// (`enabled`/`startOnBoot`/`cron` change via `flows.enable`, not Deploy), or canvas-only geometry (the
// record holds none). So re-opening a just-saved flow reads CLEAN.

import type { Flow, FlowNode } from "@/lib/flows";

/** A node reduced to its comparable, order-stable shape. */
interface NormalNode {
  id: string;
  type: string;
  needs: string[];
  config: string;
  with: string;
}

/** Stable-stringify a value so key order never registers as a change (JSON.stringify is key-insertion-
 *  ordered). Recurses objects with sorted keys; arrays keep their order (an array's order is meaningful). */
function stable(value: unknown): string {
  return JSON.stringify(sortKeys(value));
}

function sortKeys(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(sortKeys);
  if (value !== null && typeof value === "object") {
    const obj = value as Record<string, unknown>;
    const out: Record<string, unknown> = {};
    for (const k of Object.keys(obj).sort()) out[k] = sortKeys(obj[k]);
    return out;
  }
  return value;
}

function normalNode(n: FlowNode): NormalNode {
  return {
    id: n.id,
    type: n.type,
    // `needs` is a set (wiring), not a sequence — sort so a re-ordered edge list isn't "dirty".
    needs: [...(n.needs ?? [])].sort(),
    config: stable(n.config ?? {}),
    with: stable(n.with ?? {}),
  };
}

/** The comparable signature of a flow's graph: its nodes (id-sorted) in normal form. Two flows with the
 *  same signature are the same deployed graph regardless of node/edge/key ordering. */
function graphSignature(flow: Flow): string {
  const nodes = [...flow.nodes]
    .map(normalNode)
    .sort((a, b) => a.id.localeCompare(b.id));
  return stable(nodes);
}

/** True when `buffer` (the canvas edit) differs from `saved` (the deployed flow) in graph shape — the
 *  Deploy button's enabled state. Ignores version/lifecycle/geometry (see file header). */
export function flowDirty(saved: Flow, buffer: Flow): boolean {
  return graphSignature(saved) !== graphSignature(buffer);
}
