// The live generic-bus feed over SSE (widget-config-vars "Platform fix") — the motion read for a
// `bus.watch` source. Mirrors `openSeriesStream` but hits `GET /bus/stream?subject=&token=` and listens
// for `message` events carrying the published JSON payload. A cell/variable opens this once per subject
// and folds each live payload in. State is a read tool; this is motion (rule 3) — no polling.
//
// One verb: `openBusStream`. Native `EventSource`, so it returns `null` with no gateway (Tauri/tests) —
// the caller then has only its backfilled state, by design. The subject is a query param (it contains
// `/`); the token rides as `?token=` (EventSource can't set an Authorization header).

import { eventHub, liveStreamAvailable } from "@/lib/events/hub";

/** A live bus stream handle — call `close()` to stop (the hook does this on unmount). */
export interface BusStream {
  close: () => void;
}

/** Open the SSE stream for `subject`. Returns `null` when no gateway is configured (Tauri/tests). Each
 *  `message` event carries the published JSON payload verbatim (the gateway emits it as parsed data). */
export function openBusStream(
  subject: string,
  onMessage: (payload: unknown) => void,
): BusStream | null {
  if (!liveStreamAvailable()) return null;
  // Delegates to the shared event hub: the `bus:{subject}` subject rides the one multiplexed connection.
  // The subject keeps its `/`s and inner colons — the mux splits kind on the FIRST colon, and the host
  // walls the subject exactly as the dedicated route did. `event: message` payload is unchanged.
  const unsubscribe = eventHub.subscribeSubject(`bus:${subject}`, (frame) => {
    if (frame.event !== "message") return;
    try {
      onMessage(JSON.parse(frame.data));
    } catch {
      // a malformed frame never breaks the stream
    }
  });
  return { close: unsubscribe };
}
