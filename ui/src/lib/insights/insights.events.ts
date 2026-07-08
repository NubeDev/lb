// The live insight feed over SSE — the server→browser push (mirrors the gateway's
// `GET /insights/events`). The browser opens this once and receives raise/ack/resolve events on
// the workspace subject `ws/{ws}/insight/events`, which `useInsights` folds into a head refresh.
//
// One verb: `subscribeInsightEvents`. It uses the native `EventSource`, so it only runs in a real
// browser against a real gateway — in the Tauri shell and in tests there is no gateway URL, and the
// caller gets a no-op unsubscribe (live updates there come from the act→refresh round trip).

import { eventHub, liveStreamAvailable } from "@/lib/events/hub";
import type { InsightEvent } from "./insights.types";

/** Subscribe to the workspace's insight events. `onEvent` fires per raise/ack/resolve. Returns an
 *  unsubscribe function; a no-op when no gateway is configured (Tauri shell / tests / SSR). */
export function subscribeInsightEvents(
  onEvent: (event: InsightEvent) => void,
): () => void {
  if (!liveStreamAvailable()) return () => {};
  // Delegates to the shared event hub: the `insights` subject rides the one multiplexed connection.
  // `event: message` (the raise/ack/resolve feed) is byte-identical to the dedicated route's.
  return eventHub.subscribeSubject("insights", (frame) => {
    if (frame.event !== "message") return;
    try {
      onEvent(JSON.parse(frame.data) as InsightEvent);
    } catch {
      // a malformed frame never breaks the stream
    }
  });
}
