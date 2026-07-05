// The live insight feed over SSE — the server→browser push (mirrors the gateway's
// `GET /insights/events`). The browser opens this once and receives raise/ack/resolve events on
// the workspace subject `ws/{ws}/insight/events`, which `useInsights` folds into a head refresh.
//
// One verb: `subscribeInsightEvents`. It uses the native `EventSource`, so it only runs in a real
// browser against a real gateway — in the Tauri shell and in tests there is no gateway URL, and the
// caller gets a no-op unsubscribe (live updates there come from the act→refresh round trip).

import { gatewayUrl } from "@/lib/ipc/http";
import { sessionToken } from "@/lib/session/session.store";
import type { InsightEvent } from "./insights.types";

/** Subscribe to the workspace's insight events. `onEvent` fires per raise/ack/resolve. Returns an
 *  unsubscribe function; a no-op when no gateway is configured (Tauri shell / tests / SSR). */
export function subscribeInsightEvents(
  onEvent: (event: InsightEvent) => void,
): () => void {
  const base = gatewayUrl();
  if (base === "" && import.meta.env.VITE_GATEWAY_URL === undefined) return () => {};
  if (typeof EventSource === "undefined") return () => {};

  // The token rides as a query param — `EventSource` cannot set an Authorization header, and the
  // gateway's events route authenticates by `?token=` for exactly this reason (the hard wall holds:
  // workspace + caps come from the verified token; the subject is ws-scoped, no cross-ws leak).
  const url = `${base}/insights/events?token=${encodeURIComponent(sessionToken())}`;
  const es = new EventSource(url);
  es.addEventListener("message", (e) => {
    try {
      onEvent(JSON.parse((e as MessageEvent).data) as InsightEvent);
    } catch {
      // a malformed frame never breaks the stream
    }
  });
  return () => es.close();
}
