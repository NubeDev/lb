// The live generic-bus feed over SSE (widget-config-vars "Platform fix") — the motion read for a
// `bus.watch` source. Mirrors `openSeriesStream` but hits `GET /bus/stream?subject=&token=` and listens
// for `message` events carrying the published JSON payload. A cell/variable opens this once per subject
// and folds each live payload in. State is a read tool; this is motion (rule 3) — no polling.
//
// One verb: `openBusStream`. Native `EventSource`, so it returns `null` with no gateway (Tauri/tests) —
// the caller then has only its backfilled state, by design. The subject is a query param (it contains
// `/`); the token rides as `?token=` (EventSource can't set an Authorization header).

import { gatewayUrl } from "@/lib/ipc/http";
import { sessionToken } from "@/lib/session/session.store";

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
  const base = gatewayUrl();
  if (base === "" && import.meta.env.VITE_GATEWAY_URL === undefined) return null;
  if (typeof EventSource === "undefined") return null;

  const url = `${base}/bus/stream?subject=${encodeURIComponent(subject)}&token=${encodeURIComponent(
    sessionToken(),
  )}`;
  const es = new EventSource(url);

  es.addEventListener("message", (e) => {
    try {
      onMessage(JSON.parse((e as MessageEvent).data));
    } catch {
      // a malformed frame never breaks the stream
    }
  });

  return { close: () => es.close() };
}
