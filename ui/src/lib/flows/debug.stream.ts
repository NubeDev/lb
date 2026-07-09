// The live flow debug feed over SSE (debug-node-scope) — the motion read the canvas debug panel tails.
// Mirrors `openFlowRunStream`: opens `GET /flows/{flowId}/debug/stream?token=` once and listens for
// `debug` frames — each one wire message a `debug` node published (json/text/markdown, attribution
// + a `collapseBytes` hint). **Deltas-only in v1** (motion-only — no snapshot, no replay): a late
// opener sees messages from attach onward; persistence-to-disc is a named follow-up.
//
// One verb: `openFlowDebugStream`. Native `EventSource`, so it returns `null` with no gateway
// (Tauri/tests) — the caller then renders "stream unavailable", not an error. The token rides as
// `?token=` (EventSource can't set an Authorization header).

import { eventHub, liveStreamAvailable } from "@/lib/events/hub";

import type { DebugMessage } from "./flows.types";

/** A live debug stream handle — call `close()` to stop (the hook does this on unmount). */
export interface FlowDebugStream {
  close: () => void;
}

/** Open the SSE debug stream for `flowId`. Returns `null` when no gateway is configured
 *  (Tauri/tests). Each `debug` frame is one wire message a `debug` node published; a `dropped` frame
 *  is the publish-governor sentinel ("N messages were suppressed"). */
export function openFlowDebugStream(
  flowId: string,
  onMessage: (msg: DebugMessage) => void,
): FlowDebugStream | null {
  if (!liveStreamAvailable()) return null;
  // Delegates to the shared event hub: the `flow-debug:{flowId}` subject rides the one multiplexed
  // connection. Deltas-only (no snapshot), exactly as the dedicated route; `event: debug` unchanged.
  const unsubscribe = eventHub.subscribeSubject(`flow-debug:${flowId}`, (frame) => {
    if (frame.event !== "debug") return;
    try {
      onMessage(JSON.parse(frame.data) as DebugMessage);
    } catch {
      // a malformed frame never breaks the stream
    }
  });
  return { close: unsubscribe };
}
