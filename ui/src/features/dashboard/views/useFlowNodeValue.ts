// Read ONE flow node's current value from `flows.node_state` (flow-dashboard-binding-ux-scope, the
// read-back Decision). One read drives both the canvas and the dashboard — no new verb, no
// `runs.get` poll, no `series.watch` on an arbitrary node. The whole-flow node_state is fetched and
// THIS node/port is extracted:
//
//   - `kind: "output"` → the node's OUTPUT `payload` (its latest emitted value) — the read view /
//     gauge / JSON-out view binds here;
//   - `kind: "input"`  → the node's RETAINED input for `port` (`inputs[port]` per-port, else the
//     node-level `input`) — a control seeds its CURRENT state from its OWN input (not its output),
//     so a switch/slider shows true state after reload/restart, not a default.
//
// Refreshes on `refreshKey` (the canvas-cadence tick the dashboard already drives) — the v1 liveness;
// a `flows.node.watch` SSE is a later slice. A denied/missing flow degrades to `null` (honest empty).

import { useQuery } from "@tanstack/react-query";

import { getFlowNodeState } from "@/lib/flows/flows.api";
import type { NodeStateEntry } from "@/lib/flows/flows.types";
import { useDashboardWs } from "../cache/useDashboardWs";
import { flowNodeStateKey } from "../cache/queryKeys";
import { valueAtPath, type PathSeg } from "./jsonPaths";

export interface FlowNodeValue {
  value: unknown;
  rev: number | null;
  loading: boolean;
  denied: boolean;
}

/** Extract the bound slot's value from a node_state entry — **agnostic to the port NAME**, so a
 *  developer's new node type (output port `temperature`/`findings`/anything) works with no changes:
 *
 *  - `input` → the per-port retained value `inputs[port]`, falling back to the node-level retained
 *    `input` (the picker selects the slot by name; we never assume `payload`).
 *  - `output` → the recorded envelope's `value[port]` field (the engine records each emitted output
 *    port under its own name). Falls back to `value.payload` (the common primary slot), then the whole
 *    value (a node that recorded a bare scalar, not an envelope object).
 *  - `output-envelope` → the whole recorded value (the JSON view's "show the envelope" mode). */
export type FlowValueKind = "input" | "output" | "output-envelope";

export function extractFlowValue(
  entry: NodeStateEntry | undefined,
  kind: FlowValueKind,
  port: string,
  path?: PathSeg[],
): unknown {
  if (!entry) return null;
  if (kind === "input") {
    const perPort = entry.inputs?.[port];
    if (perPort !== undefined) return perPort;
    return entry.input ?? null;
  }
  const v = entry.value;
  // An explicit JSON PATH (the visual builder's selection) is AUTHORITATIVE — extract exactly that
  // field from the node's WHOLE recorded value, INCLUDING the empty path `[]` ("(whole value)"). This
  // keeps the picker's preview and the rendered widget identical. `payload`, `payload.cron_ts`,
  // `items[0].name`, … ; a missing field → null (honest, never stale).
  if (path !== undefined) return valueAtPath(v, path) ?? null;
  if (kind === "output-envelope") return v ?? null;
  // No path authored → the port default (back-compat with the simple binding).
  if (v && typeof v === "object" && !Array.isArray(v)) {
    const obj = v as Record<string, unknown>;
    // The SELECTED output port first (agnostic), then `payload` (the usual primary), then nothing.
    if (port in obj) return obj[port];
    if ("payload" in obj) return obj.payload;
    return null;
  }
  // A bare recorded scalar (no envelope object) — the value itself is the output.
  return v ?? null;
}

/** Read flow `flowId`'s node `node` value for `port` (input or output) — optionally extracting a JSON
 *  `path` into the value (the visual builder's selection) — re-reading on `refreshKey`. */
export function useFlowNodeValue(
  flowId: string | undefined,
  node: string | undefined,
  port: string,
  kind: FlowValueKind,
  refreshKey = 0,
  path?: PathSeg[],
): FlowNodeValue {
  const ws = useDashboardWs();

  // The WHOLE-flow node_state read is the shared cache entry, keyed on (ws, flow, tick) — NOT the node/
  // port/path. So N cells on one flow issue ONE `flows.node_state` read (scope goal 4); each cell slices
  // its OWN node/port/path client-side from the shared result below (a re-slice, not a re-fetch).
  const query = useQuery({
    queryKey: flowNodeStateKey(ws, flowId ?? "", refreshKey),
    enabled: !!flowId && !!node,
    queryFn: () => getFlowNodeState(flowId!),
  });

  if (!flowId || !node) return { value: null, rev: null, loading: false, denied: true };
  if (query.isError) return { value: null, rev: null, loading: false, denied: true };
  if (query.isLoading || !query.data) return { value: null, rev: null, loading: true, denied: false };
  const entry = query.data.nodes.find((n) => n.node === node);
  return {
    value: extractFlowValue(entry, kind, port, path),
    rev: entry?.rev ?? null,
    loading: false,
    denied: false,
  };
}
