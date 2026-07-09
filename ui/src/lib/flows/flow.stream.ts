// The live flow-run settle feed over SSE (flow-runtime-control-scope) — the motion read the canvas
// folds to colour nodes as they settle, the replacement for polling `flows.runs.get`. Mirrors
// `openBusStream`/`openSeriesStream`: opens `GET /flows/runs/{runId}/stream?token=` once and listens
// for a `snapshot` event (the run as of attach) then `flow` events (each a `node-settled` or
// `run-finished` delta). State is a record read; this is motion (rule 3) — no polling.
//
// One verb: `openFlowRunStream`. Native `EventSource`, so it returns `null` with no gateway
// (Tauri/tests) — the caller then falls back to its bounded `flows.runs.get` poll, by design. The
// token rides as `?token=` (EventSource can't set an Authorization header).

import { eventHub, liveStreamAvailable } from "@/lib/events/hub";

import type { FlowRunSnapshot } from "./flows.types";

/** One live settle delta off the stream: a node went terminal, or the run finished. */
export type FlowStreamEvent =
  | { kind: "node-settled"; id: string; outcome: string; output?: unknown; error?: string | null }
  | { kind: "run-finished"; status: string };

/** A live flow-run stream handle — call `close()` to stop (the hook does this on unmount). */
export interface FlowRunStream {
  close: () => void;
}

/** Open the SSE stream for `runId`. Returns `null` when no gateway is configured (Tauri/tests) so the
 *  caller can fall back to the poll. The first frame is the `snapshot`; each subsequent `flow` frame
 *  is a `node-settled`/`run-finished` delta. */
export function openFlowRunStream(
  runId: string,
  onSnapshot: (snap: FlowRunSnapshot) => void,
  onEvent: (event: FlowStreamEvent) => void,
): FlowRunStream | null {
  if (!liveStreamAvailable()) return null;
  // Delegates to the shared event hub: the `flow-run:{runId}` subject rides the one multiplexed
  // connection. The `snapshot` then `flow` frames are byte-identical to the dedicated route's.
  const unsubscribe = eventHub.subscribeSubject(`flow-run:${runId}`, (frame) => {
    try {
      if (frame.event === "snapshot") {
        onSnapshot(JSON.parse(frame.data) as FlowRunSnapshot);
      } else if (frame.event === "flow") {
        onEvent(JSON.parse(frame.data) as FlowStreamEvent);
      }
    } catch {
      // a malformed frame never breaks the stream
    }
  });
  return { close: unsubscribe };
}
