// Recognise a flow-bound control/read and recover its `{flowId, node, port}` (flow-dashboard-binding-
// ux-scope). The picker emits `flows.inject` Actions (input ports) and `flows.node_state` Sources
// (output ports); a control reads its CURRENT state back from its OWN input via these coordinates, so
// the seam that produced the binding also feeds the read-back — no second source to author.

import type { Action, Source } from "@/lib/dashboard";
import { asPath, type PathSeg } from "./jsonPaths";

export interface FlowBinding {
  flowId: string;
  node: string;
  port: string;
  /** An optional JSON path INTO the node's value (the visual builder's selection) — the read view
   *  extracts exactly this field (object / array / nested). Empty → the port's value. */
  path?: PathSeg[];
}

/** A flow inject Action → its `{flowId, node, port}`; non-flow actions → null. */
export function flowBindingOfAction(action: Action | undefined): FlowBinding | null {
  if (action?.tool !== "flows.inject") return null;
  const t = action.argsTemplate ?? {};
  const flowId = t.id as string | undefined;
  const node = t.node as string | undefined;
  const port = (t.port as string | undefined) ?? "payload";
  if (!flowId || !node) return null;
  return { flowId, node, port };
}

/** A flow `flows.node_state` read Source → its `{flowId, node, port}`; non-flow sources → null. The
 *  picker stashes the node/port under `__flowNode`/`__flowPort` (the whole-flow read is by `id`). */
export function flowBindingOfSource(source: Source | undefined): FlowBinding | null {
  if (source?.tool !== "flows.node_state") return null;
  const a = source.args ?? {};
  const flowId = a.id as string | undefined;
  const node = a.__flowNode as string | undefined;
  const port = (a.__flowPort as string | undefined) ?? "payload";
  if (!flowId || !node) return null;
  // `path` is UNDEFINED when no `__flowPath` key was authored (use the port default), vs an explicit
  // `[]` (the visual builder's "(whole value)" selection) — the two must read differently.
  const hasPath = typeof a === "object" && a !== null && "__flowPath" in a;
  return { flowId, node, port, path: hasPath ? asPath(a.__flowPath) : undefined };
}
