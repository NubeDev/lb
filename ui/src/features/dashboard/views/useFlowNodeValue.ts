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

import { useEffect, useState } from "react";

import { getFlowNodeState } from "@/lib/flows/flows.api";
import type { NodeStateEntry } from "@/lib/flows/flows.types";
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
  const [state, setState] = useState<FlowNodeValue>({
    value: null,
    rev: null,
    loading: true,
    denied: false,
  });

  useEffect(() => {
    let cancelled = false;
    if (!flowId || !node) {
      setState({ value: null, rev: null, loading: false, denied: true });
      return;
    }
    (async () => {
      try {
        const st = await getFlowNodeState(flowId);
        if (cancelled) return;
        const entry = st.nodes.find((n) => n.node === node);
        setState({
          value: extractFlowValue(entry, kind, port, path),
          rev: entry?.rev ?? null,
          loading: false,
          denied: false,
        });
      } catch {
        if (cancelled) return;
        setState({ value: null, rev: null, loading: false, denied: true });
      }
    })();
    return () => {
      cancelled = true;
    };
    // `path` is folded into the dep key as a string so a new selection re-extracts (arrays differ by ref).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [flowId, node, port, kind, refreshKey, JSON.stringify(path ?? null)]);

  return state;
}
